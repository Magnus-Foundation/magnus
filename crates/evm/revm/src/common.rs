use crate::MagnusTxEnv;
use alloy_consensus::transaction::{Either, Recovered};
use alloy_primitives::{Address, Bytes, LogData, TxKind, U256};
use alloy_sol_types::SolCall;
use core::marker::PhantomData;
use revm::{
    Database,
    context::JournalTr,
    state::{AccountInfo, Bytecode},
};
use magnus_chainspec::hardfork::MagnusHardfork;
use magnus_contracts::precompiles::FeeManagerError;
use magnus_precompiles::{
    MIP_FEE_MANAGER_ADDRESS,
    error::{Result as MagnusResult, MagnusPrecompileError},
    storage::{Handler, PrecompileStorageProvider, StorageCtx},
    mip_fee_manager::MipFeeManager,
    mip20::{IMIP20, MIP20Token},
    mip403_registry::{AuthRole, MIP403Registry},
};
use magnus_primitives::{MagnusAddressExt, MagnusTxEnvelope};

/// Returns true if the calldata is for a MIP-20 function that should trigger fee token inference.
/// Only `transfer`, `transferWithMemo`, and `distributeReward` qualify.
fn is_tip20_fee_inference_call(input: &[u8]) -> bool {
    input.first_chunk::<4>().is_some_and(|&s| {
        matches!(
            s,
            IMIP20::transferCall::SELECTOR
                | IMIP20::transferWithMemoCall::SELECTOR
                | IMIP20::distributeRewardCall::SELECTOR
        )
    })
}

/// Helper trait to abstract over different representations of Magnus transactions.
#[auto_impl::auto_impl(&, Arc)]
pub trait MagnusTx {
    /// Returns the transaction's `feeToken` field, if configured.
    fn fee_token(&self) -> Option<Address>;

    /// Returns true if this is an AA transaction.
    fn is_aa(&self) -> bool;

    /// Returns an iterator over the transaction's calls.
    fn calls(&self) -> impl Iterator<Item = (TxKind, &Bytes)>;

    /// Returns the transaction's caller address.
    fn caller(&self) -> Address;
}

impl MagnusTx for MagnusTxEnv {
    fn fee_token(&self) -> Option<Address> {
        self.fee_token
    }

    fn is_aa(&self) -> bool {
        self.magnus_tx_env.is_some()
    }

    fn calls(&self) -> impl Iterator<Item = (TxKind, &Bytes)> {
        if let Some(aa) = self.magnus_tx_env.as_ref() {
            Either::Left(aa.aa_calls.iter().map(|call| (call.to, &call.input)))
        } else {
            Either::Right(core::iter::once((self.inner.kind, &self.inner.data)))
        }
    }

    fn caller(&self) -> Address {
        self.inner.caller
    }
}

impl MagnusTx for Recovered<MagnusTxEnvelope> {
    fn fee_token(&self) -> Option<Address> {
        self.inner().fee_token()
    }

    fn is_aa(&self) -> bool {
        self.inner().is_aa()
    }

    fn calls(&self) -> impl Iterator<Item = (TxKind, &Bytes)> {
        self.inner().calls()
    }

    fn caller(&self) -> Address {
        self.signer()
    }
}

/// Helper trait to perform Magnus-specific operations on top of different state providers.
///
/// We provide blanket implementations for revm database, journal and reth state provider.
///
/// The generic marker is used as a workaround to avoid conflicting implementations.
pub trait MagnusStateAccess<M = ()> {
    /// Error type returned by storage operations.
    type Error: core::fmt::Display;

    /// Returns [`AccountInfo`] for the given address.
    fn basic(&mut self, address: Address) -> Result<AccountInfo, Self::Error>;

    /// Returns the storage value for the given address and key.
    fn sload(&mut self, address: Address, key: U256) -> Result<U256, Self::Error>;

    /// Returns a read-only storage provider for the given spec.
    fn with_read_only_storage_ctx<R>(&mut self, spec: MagnusHardfork, f: impl FnOnce() -> R) -> R
    where
        Self: Sized,
    {
        StorageCtx::enter(&mut ReadOnlyStorageProvider::new(self, spec), f)
    }

    /// Resolves the fee token. After T4 there is no per-user preference and no
    /// default fallback: tx.fee_token wins, then direct MIP-20 inference, then
    /// the router selector registry; anything else surfaces FeeTokenNotInferable.
    fn get_fee_token(
        &mut self,
        tx: impl MagnusTx,
        fee_payer: Address,
        spec: MagnusHardfork,
    ) -> MagnusResult<Address>
    where
        Self: Sized,
    {
        if let Some(fee_token) = tx.fee_token() {
            return Ok(fee_token);
        }

        // Direct MIP-20 inference from a same-token transfer/transferWithMemo/
        // distributeReward batch.
        if let Some(to) = tx.calls().next().and_then(|(kind, _)| kind.to().copied()) {
            let can_infer_tip20 = if tx.is_aa() && fee_payer != tx.caller() {
                false
            } else {
                tx.calls().all(|(kind, input)| {
                    kind.to() == Some(&to) && is_tip20_fee_inference_call(input)
                })
            };
            if can_infer_tip20 && self.is_valid_fee_token(spec, to)? {
                return Ok(to);
            }
        }

        // Router selector registry: single-call (or AA single-call) txs
        // resolve their fee token via the descriptor's argument index.
        let mut calls = tx.calls();
        if let Some((kind, input)) = calls.next()
            && (!tx.is_aa() || calls.next().is_none())
            && let Some(router) = kind.to().copied()
            && let Some(selector) = input.first_chunk::<4>().copied()
            && let Some(token) = self.lookup_router_fee_token(spec, router, selector, input)?
        {
            return Ok(token);
        }

        Err(MagnusPrecompileError::FeeManagerError(
            FeeManagerError::fee_token_not_inferable(),
        ))
    }

    /// Looks up `(router, selector)` in the on-chain router registry and, if
    /// registered, decodes the token argument from `calldata` at the descriptor's
    /// arg index. Returns `Ok(None)` when the selector is not registered;
    /// `Err(CalldataDecodeFailed)` when calldata is malformed.
    fn lookup_router_fee_token(
        &mut self,
        spec: MagnusHardfork,
        router: Address,
        selector: [u8; 4],
        calldata: &[u8],
    ) -> MagnusResult<Option<Address>>
    where
        Self: Sized,
    {
        self.with_read_only_storage_ctx(spec, || {
            let fee_manager = MipFeeManager::new();
            let (registered, arg_index) = fee_manager
                .lookup_router_selector(router, alloy_primitives::FixedBytes::from(selector))?;
            if !registered {
                return Ok(None);
            }
            let token = fee_manager.decode_router_token_arg(calldata, arg_index)?;
            Ok(Some(token))
        })
    }

    /// Checks if the given MIP20 token has USD currency.
    ///
    /// IMPORTANT: Caller must ensure `fee_token` has a valid MIP20 prefix.
    fn is_tip20_usd(&mut self, spec: MagnusHardfork, fee_token: Address) -> MagnusResult<bool>
    where
        Self: Sized,
    {
        self.with_read_only_storage_ctx(spec, || {
            // SAFETY: caller must ensure prefix is already checked
            let token = MIP20Token::from_address_unchecked(fee_token);
            Ok(token.currency.len()? == 3 && token.currency.read()?.as_str() == "USD")
        })
    }

    /// Checks if the given token can be used as a fee token.
    fn is_valid_fee_token(&mut self, spec: MagnusHardfork, fee_token: Address) -> MagnusResult<bool>
    where
        Self: Sized,
    {
        // Must have MIP20 prefix to be a valid fee token
        if !fee_token.is_tip20() {
            return Ok(false);
        }

        // Ensure the currency is USD
        self.is_tip20_usd(spec, fee_token)
    }

    /// Checks if a fee token is paused.
    fn is_fee_token_paused(&mut self, spec: MagnusHardfork, fee_token: Address) -> MagnusResult<bool>
    where
        Self: Sized,
    {
        self.with_read_only_storage_ctx(spec, || {
            let token = MIP20Token::from_address(fee_token)?;
            token.paused()
        })
    }

    /// Checks if the fee payer can transfer the fee token to the fee manager.
    fn can_fee_payer_transfer(
        &mut self,
        fee_token: Address,
        fee_payer: Address,
        spec: MagnusHardfork,
    ) -> MagnusResult<bool>
    where
        Self: Sized,
    {
        self.with_read_only_storage_ctx(spec, || {
            let token = MIP20Token::from_address(fee_token)?;
            if spec.is_t1c() {
                // Check both the fee payer and the fee manager is authorized
                token.is_transfer_authorized(fee_payer, MIP_FEE_MANAGER_ADDRESS)
            } else {
                let policy_id = token.transfer_policy_id.read()?;
                MIP403Registry::new().is_authorized_as(policy_id, fee_payer, AuthRole::sender())
            }
        })
    }

    /// Returns the balance of the given token for the given account.
    ///
    /// IMPORTANT: the caller must ensure `token` is a valid MIP20Token address.
    fn get_token_balance(
        &mut self,
        token: Address,
        account: Address,
        spec: MagnusHardfork,
    ) -> MagnusResult<U256>
    where
        Self: Sized,
    {
        self.with_read_only_storage_ctx(spec, || {
            // Load the token balance for the given account.
            MIP20Token::from_address(token)?.balances[account].read()
        })
    }
}

impl<DB: Database> MagnusStateAccess<()> for DB {
    type Error = DB::Error;

    fn basic(&mut self, address: Address) -> Result<AccountInfo, Self::Error> {
        self.basic(address).map(Option::unwrap_or_default)
    }

    fn sload(&mut self, address: Address, key: U256) -> Result<U256, Self::Error> {
        self.storage(address, key)
    }
}

impl<T: JournalTr> MagnusStateAccess<((), ())> for T {
    type Error = <T::Database as Database>::Error;

    fn basic(&mut self, address: Address) -> Result<AccountInfo, Self::Error> {
        self.load_account(address).map(|s| s.data.info.clone())
    }

    fn sload(&mut self, address: Address, key: U256) -> Result<U256, Self::Error> {
        JournalTr::sload(self, address, key).map(|s| s.data)
    }
}

#[cfg(feature = "reth")]
impl<T: reth_storage_api::StateProvider> MagnusStateAccess<((), (), ())> for T {
    type Error = reth_evm::execute::ProviderError;

    fn basic(&mut self, address: Address) -> Result<AccountInfo, Self::Error> {
        self.basic_account(&address)
            .map(Option::unwrap_or_default)
            .map(Into::into)
    }

    fn sload(&mut self, address: Address, key: U256) -> Result<U256, Self::Error> {
        self.storage(address, key.into())
            .map(Option::unwrap_or_default)
    }
}

/// Read-only storage provider that wraps a `MagnusStateAccess`.
///
/// Implements `PrecompileStorageProvider` by delegating read operations to the backend
/// and returning errors for write operations.
///
/// The marker generic `M` selects which `MagnusStateAccess<M>` impl to use for the backend.
struct ReadOnlyStorageProvider<'a, S, M = ()> {
    state: &'a mut S,
    spec: MagnusHardfork,
    _marker: PhantomData<M>,
}

impl<'a, S, M> ReadOnlyStorageProvider<'a, S, M>
where
    S: MagnusStateAccess<M>,
{
    /// Creates a new read-only storage provider.
    fn new(state: &'a mut S, spec: MagnusHardfork) -> Self {
        Self {
            state,
            spec,
            _marker: PhantomData,
        }
    }
}

impl<S, M> PrecompileStorageProvider for ReadOnlyStorageProvider<'_, S, M>
where
    S: MagnusStateAccess<M>,
{
    fn spec(&self) -> MagnusHardfork {
        self.spec
    }

    fn is_static(&self) -> bool {
        // read-only operations should always be static
        true
    }

    fn sload(&mut self, address: Address, key: U256) -> MagnusResult<U256> {
        let _ = self
            .state
            .basic(address)
            .map_err(|e| MagnusPrecompileError::Fatal(e.to_string()))?;
        self.state
            .sload(address, key)
            .map_err(|e| MagnusPrecompileError::Fatal(e.to_string()))
    }

    fn with_account_info(
        &mut self,
        address: Address,
        f: &mut dyn FnMut(&AccountInfo),
    ) -> MagnusResult<()> {
        let info = self
            .state
            .basic(address)
            .map_err(|e| MagnusPrecompileError::Fatal(e.to_string()))?;
        f(&info);
        Ok(())
    }

    // No-op methods are unimplemented in read-only context.
    fn chain_id(&self) -> u64 {
        unreachable!("'chain_id' not implemented in read-only context yet")
    }

    fn timestamp(&self) -> U256 {
        unreachable!("'timestamp' not implemented in read-only context yet")
    }

    fn beneficiary(&self) -> Address {
        unreachable!("'beneficiary' not implemented in read-only context yet")
    }

    fn block_number(&self) -> u64 {
        unreachable!("'block_number' not implemented in read-only context yet")
    }

    fn tload(&mut self, _: Address, _: U256) -> MagnusResult<U256> {
        unreachable!("'tload' not implemented in read-only context yet")
    }

    fn gas_used(&self) -> u64 {
        unreachable!("'gas_used' not implemented in read-only context yet")
    }

    fn gas_refunded(&self) -> i64 {
        unreachable!("'gas_refunded' not implemented in read-only context yet")
    }

    fn reservoir(&self) -> u64 {
        unreachable!("'reservoir' not implemented in read-only context yet")
    }

    // Write operations are not supported in read-only context
    fn sstore(&mut self, _: Address, _: U256, _: U256) -> MagnusResult<()> {
        unreachable!("'sstore' not supported in read-only context")
    }

    fn set_code(&mut self, _: Address, _: Bytecode) -> MagnusResult<()> {
        unreachable!("'set_code' not supported in read-only context")
    }

    fn emit_event(&mut self, _: Address, _: LogData) -> MagnusResult<()> {
        unreachable!("'emit_event' not supported in read-only context")
    }

    fn tstore(&mut self, _: Address, _: U256, _: U256) -> MagnusResult<()> {
        unreachable!("'tstore' not supported in read-only context")
    }

    fn deduct_gas(&mut self, _: u64) -> MagnusResult<()> {
        unreachable!("'deduct_gas' not supported in read-only context")
    }

    fn refund_gas(&mut self, _: i64) {
        unreachable!("'refund_gas' not supported in read-only context")
    }

    fn checkpoint(&mut self) -> revm::context::journaled_state::JournalCheckpoint {
        unreachable!("'checkpoint' not supported in read-only context")
    }

    fn checkpoint_commit(&mut self, _: revm::context::journaled_state::JournalCheckpoint) {
        unreachable!("'checkpoint_commit' not supported in read-only context")
    }

    fn checkpoint_revert(&mut self, _: revm::context::journaled_state::JournalCheckpoint) {
        unreachable!("'checkpoint_revert' not supported in read-only context")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MagnusBlockEnv, MagnusEvm};
    use alloy_primitives::uint;
    use reth_evm::EvmInternals;
    use revm::{
        Context, MainContext, context::TxEnv, database::EmptyDB,
    };
    use magnus_precompiles::{
        MAGNUS_USD_ADDRESS,
        storage::{StorageCtx, evm::EvmPrecompileStorageProvider},
        test_util::MIP20Setup,
        mip20::{IRolesAuth::*, IMIP20::*, MIP20Token, slots as mip20_slots},
        mip403_registry::{IMIP403Registry, MIP403Registry},
    };

    #[test]
    fn test_get_fee_token_fee_token_set() -> eyre::Result<()> {
        let caller = Address::random();
        let fee_token = Address::random();

        let tx_env = TxEnv {
            data: Bytes::new(),
            caller,
            ..Default::default()
        };
        let tx = MagnusTxEnv {
            inner: tx_env,
            fee_token: Some(fee_token),
            ..Default::default()
        };

        let mut db = EmptyDB::default();
        let token = db.get_fee_token(tx, caller, MagnusHardfork::Genesis)?;
        assert_eq!(token, fee_token);
        Ok(())
    }

    #[test]
    fn test_get_fee_token_no_preferences_returns_inferable_error() -> eyre::Result<()> {
        // T4: no fee_token, no MIP-20 inference target, no router match.
        let caller = Address::random();
        let tx_env = TxEnv {
            caller,
            ..Default::default()
        };
        let tx = MagnusTxEnv {
            inner: tx_env,
            ..Default::default()
        };

        let mut db = EmptyDB::default();
        let err = db
            .get_fee_token(tx, caller, MagnusHardfork::T4)
            .unwrap_err();
        assert!(matches!(
            err,
            MagnusPrecompileError::FeeManagerError(
                magnus_contracts::precompiles::FeeManagerError::FeeTokenNotInferable(_)
            )
        ));
        Ok(())
    }

    #[test]
    fn test_read_token_balance_typed_storage() -> eyre::Result<()> {
        let token_address = MAGNUS_USD_ADDRESS;
        let account = Address::random();
        let expected_balance = U256::from(1000u64);

        // Set up CacheDB with balance
        let mut db = revm::database::CacheDB::new(EmptyDB::default());
        let balance_slot = MIP20Token::from_address(token_address)?.balances[account].slot();
        db.insert_account_storage(token_address, balance_slot, expected_balance)?;

        // Read balance using typed storage
        let balance = db.get_token_balance(token_address, account, MagnusHardfork::Genesis)?;
        assert_eq!(balance, expected_balance);

        Ok(())
    }

    #[test]
    fn test_is_tip20_fee_inference_call() {
        // Allowed selectors
        assert!(is_tip20_fee_inference_call(&transferCall::SELECTOR));
        assert!(is_tip20_fee_inference_call(&transferWithMemoCall::SELECTOR));
        assert!(is_tip20_fee_inference_call(&distributeRewardCall::SELECTOR));

        // Disallowed selectors
        assert!(!is_tip20_fee_inference_call(&grantRoleCall::SELECTOR));
        assert!(!is_tip20_fee_inference_call(&mintCall::SELECTOR));
        assert!(!is_tip20_fee_inference_call(&approveCall::SELECTOR));

        // Edge cases
        assert!(!is_tip20_fee_inference_call(&[]));
        assert!(!is_tip20_fee_inference_call(&[0x00, 0x01, 0x02]));
    }

    #[test]
    fn test_is_fee_token_paused() -> eyre::Result<()> {
        let token_address = MAGNUS_USD_ADDRESS;
        let mut db = revm::database::CacheDB::new(EmptyDB::default());

        // Default (unpaused) returns false
        assert!(!db.is_fee_token_paused(MagnusHardfork::Genesis, token_address)?);

        // Set paused=true
        db.insert_account_storage(token_address, mip20_slots::PAUSED, U256::from(1))?;
        assert!(db.is_fee_token_paused(MagnusHardfork::Genesis, token_address)?);

        Ok(())
    }

    #[test]
    fn test_is_tip20_usd() -> eyre::Result<()> {
        let fee_token = MAGNUS_USD_ADDRESS;

        // Short string encoding: left-aligned data + length*2 in LSB
        let cases: &[(U256, bool, &str)] = &[
            // "USD" = 0x555344, len=3, LSB=6 -> true
            (
                uint!(0x5553440000000000000000000000000000000000000000000000000000000006_U256),
                true,
                "USD",
            ),
            // "EUR" = 0x455552, len=3, LSB=6 -> false (wrong content)
            (
                uint!(0x4555520000000000000000000000000000000000000000000000000000000006_U256),
                false,
                "EUR",
            ),
            // "US" = 0x5553, len=2, LSB=4 -> false (wrong length)
            (
                uint!(0x5553000000000000000000000000000000000000000000000000000000000004_U256),
                false,
                "US",
            ),
            // empty -> false
            (U256::ZERO, false, "empty"),
        ];

        for (currency_value, expected, label) in cases {
            let mut db = revm::database::CacheDB::new(EmptyDB::default());
            db.insert_account_storage(fee_token, mip20_slots::CURRENCY, *currency_value)?;

            let is_usd = db.is_tip20_usd(MagnusHardfork::Genesis, fee_token)?;
            assert_eq!(is_usd, *expected, "currency '{label}' failed");
        }

        Ok(())
    }

    #[test]
    fn test_can_fee_payer_transfer_t1c() -> eyre::Result<()> {
        let admin = Address::random();
        let fee_payer = Address::random();
        let db = revm::database::CacheDB::new(EmptyDB::new());
        let mut evm = MagnusEvm::new(
            Context::mainnet()
                .with_db(db)
                .with_block(MagnusBlockEnv::default())
                .with_cfg(Default::default())
                .with_tx(Default::default()),
            (),
        );

        // Set up token with whitelist policy
        let policy_id = {
            let ctx = &mut evm.ctx;
            let internals =
                EvmInternals::new(&mut ctx.journaled_state, &ctx.block, &ctx.cfg, &ctx.tx);
            let mut provider = EvmPrecompileStorageProvider::new_max_gas(internals, &ctx.cfg);
            StorageCtx::enter(&mut provider, || -> eyre::Result<u64> {
                MIP20Setup::magnus_usd(admin).apply()?;
                let mut registry = MIP403Registry::new();
                registry.initialize()?;

                let policy_id = registry.create_policy(
                    admin,
                    IMIP403Registry::createPolicyCall {
                        admin,
                        policyType: IMIP403Registry::PolicyType::WHITELIST,
                    },
                )?;
                MIP20Token::from_address(MAGNUS_USD_ADDRESS)?.change_transfer_policy_id(
                    admin,
                    IMIP20::changeTransferPolicyIdCall {
                        newPolicyId: policy_id,
                    },
                )?;
                registry.modify_policy_whitelist(
                    admin,
                    IMIP403Registry::modifyPolicyWhitelistCall {
                        policyId: policy_id,
                        account: fee_payer,
                        allowed: true,
                    },
                )?;
                Ok(policy_id)
            })?
        };

        assert!(evm.ctx.journaled_state.can_fee_payer_transfer(
            MAGNUS_USD_ADDRESS,
            fee_payer,
            MagnusHardfork::T1B
        )?);

        // Post T1C fails if fee payer not authorized
        assert!(!evm.ctx.journaled_state.can_fee_payer_transfer(
            MAGNUS_USD_ADDRESS,
            fee_payer,
            MagnusHardfork::T1C
        )?);

        // Whitelist FeeManager
        {
            let ctx = &mut evm.ctx;
            let internals =
                EvmInternals::new(&mut ctx.journaled_state, &ctx.block, &ctx.cfg, &ctx.tx);
            let mut provider = EvmPrecompileStorageProvider::new_max_gas(internals, &ctx.cfg);
            StorageCtx::enter(&mut provider, || {
                MIP403Registry::new().modify_policy_whitelist(
                    admin,
                    IMIP403Registry::modifyPolicyWhitelistCall {
                        policyId: policy_id,
                        account: MIP_FEE_MANAGER_ADDRESS,
                        allowed: true,
                    },
                )
            })?;
        }

        assert!(evm.ctx.journaled_state.can_fee_payer_transfer(
            MAGNUS_USD_ADDRESS,
            fee_payer,
            MagnusHardfork::T1B
        )?);

        assert!(evm.ctx.journaled_state.can_fee_payer_transfer(
            MAGNUS_USD_ADDRESS,
            fee_payer,
            MagnusHardfork::T1C
        )?);

        Ok(())
    }
}
