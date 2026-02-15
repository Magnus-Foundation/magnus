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
use magnus_contracts::precompiles::{
    DEFAULT_FEE_TOKEN, IFeeManager, IStablecoinDEX, IMIP403Registry, STABLECOIN_DEX_ADDRESS,
};
use magnus_precompile_registry::{
    MIP_FEE_MANAGER_ADDRESS,
    error::{Result as MagnusResult, MagnusPrecompileError},
    storage::{Handler, PrecompileStorageProvider, StorageCtx},
    mip_fee_manager::MipFeeManager,
    mip20::{IMIP20, MIP20Token, is_mip20_prefix},
    mip403_registry::MIP403Registry,
};
use magnus_primitives::MagnusTxEnvelope;

/// Returns true if the calldata is for a MIP-20 function that should trigger fee token inference.
/// Only `transfer`, `transferWithMemo`, and `distributeReward` qualify.
fn is_mip20_fee_inference_call(input: &[u8]) -> bool {
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

    /// Resolves user-level or transaction-level fee token preference.
    fn get_fee_token(
        &mut self,
        tx: impl MagnusTx,
        fee_payer: Address,
        spec: MagnusHardfork,
    ) -> MagnusResult<Address>
    where
        Self: Sized,
    {
        // If there is a fee token explicitly set on the tx type, use that.
        if let Some(fee_token) = tx.fee_token() {
            return Ok(fee_token);
        }

        // If the fee payer is also the msg.sender and the transaction is calling FeeManager to set a
        // new preference, the newly set preference should be used immediately instead of the
        // previously stored one
        if !tx.is_aa()
            && fee_payer == tx.caller()
            && let Some((kind, input)) = tx.calls().next()
            && kind.to() == Some(&MIP_FEE_MANAGER_ADDRESS)
            && let Ok(call) = IFeeManager::setUserTokenCall::abi_decode(input)
        {
            return Ok(call.token);
        }

        // Check stored user token preference
        let user_token = self.with_read_only_storage_ctx(spec, || {
            // ensure MIP_FEE_MANAGER_ADDRESS is loaded
            MipFeeManager::new().user_tokens[fee_payer].read()
        })?;

        if !user_token.is_zero() {
            return Ok(user_token);
        }

        // Check if the fee can be inferred from the MIP20 token being called
        if let Some(to) = tx.calls().next().and_then(|(kind, _)| kind.to().copied()) {
            let can_infer_mip20 =
                // AA txs only when fee_payer == tx.origin.
                if tx.is_aa() && fee_payer != tx.caller() {
                    false
                }
                // Otherwise, restricted to transfer/transferWithMemo/distributeReward,
                else {
                    tx.calls().all(|(kind, input)| {
                        kind.to() == Some(&to) && is_mip20_fee_inference_call(input)
                    })
                }
            ;

            if can_infer_mip20 && self.is_valid_fee_token(spec, to)? {
                return Ok(to);
            }
        }

        // If calling swapExactAmountOut() or swapExactAmountIn() on the Stablecoin DEX,
        // use the input token as the fee token (the token that will be pulled from the user).
        // For AA transactions, this only applies if there's exactly one call.
        let mut calls = tx.calls();
        if let Some((kind, input)) = calls.next()
            && kind.to() == Some(&STABLECOIN_DEX_ADDRESS)
            && (!tx.is_aa() || calls.next().is_none())
        {
            if let Ok(call) = IStablecoinDEX::swapExactAmountInCall::abi_decode(input)
                && self.is_valid_fee_token(spec, call.tokenIn)?
            {
                return Ok(call.tokenIn);
            } else if let Ok(call) = IStablecoinDEX::swapExactAmountOutCall::abi_decode(input)
                && self.is_valid_fee_token(spec, call.tokenIn)?
            {
                return Ok(call.tokenIn);
            }
        }

        // If no fee token is found, default to the first deployed MIP20
        Ok(DEFAULT_FEE_TOKEN)
    }

    /// Checks if the given token can be used as a fee token.
    fn is_valid_fee_token(&mut self, spec: MagnusHardfork, fee_token: Address) -> MagnusResult<bool>
    where
        Self: Sized,
    {
        // Must have MIP20 prefix to be a valid fee token
        if !is_mip20_prefix(fee_token) {
            return Ok(false);
        }

        // Ensure the currency is USD
        // load fee token account to ensure that we can load storage for it.
        self.with_read_only_storage_ctx(spec, || {
            // SAFETY: prefix already checked above
            let token = MIP20Token::from_address(fee_token)?;
            Ok(token.currency.len()? == 3 && token.currency.read()?.as_str() == "USD")
        })
    }

    /// Checks if the fee payer can transfer a given token (is not blacklisted).
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
            // Ensure the fee payer is not blacklisted
            let transfer_policy_id = MIP20Token::from_address(fee_token)?
                .transfer_policy_id
                .read()?;
            MIP403Registry::new().is_authorized(IMIP403Registry::isAuthorizedCall {
                policyId: transfer_policy_id,
                user: fee_payer,
            })
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

    fn tload(&mut self, _: Address, _: U256) -> MagnusResult<U256> {
        unreachable!("'tload' not implemented in read-only context yet")
    }

    fn gas_used(&self) -> u64 {
        unreachable!("'gas_used' not implemented in read-only context yet")
    }

    fn gas_refunded(&self) -> i64 {
        unreachable!("'gas_refunded' not implemented in read-only context yet")
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::address;
    use revm::{context::TxEnv, database::EmptyDB, interpreter::instructions::utility::IntoU256};
    use magnus_precompile_registry::PATH_USD_ADDRESS;

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
    fn test_get_fee_token_fee_manager() -> eyre::Result<()> {
        let caller = Address::random();
        let token = Address::random();

        let call = IFeeManager::setUserTokenCall { token };
        let tx_env = TxEnv {
            data: call.abi_encode().into(),
            kind: TxKind::Call(MIP_FEE_MANAGER_ADDRESS),
            caller,
            ..Default::default()
        };
        let tx = MagnusTxEnv {
            inner: tx_env,
            ..Default::default()
        };

        let mut db = EmptyDB::default();
        let result_token = db.get_fee_token(tx, caller, MagnusHardfork::Genesis)?;
        assert_eq!(result_token, token);
        Ok(())
    }

    #[test]
    fn test_get_fee_token_user_token_set() -> eyre::Result<()> {
        let caller = Address::random();
        let user_token = Address::random();

        // Set user stored token preference in the FeeManager
        let mut db = revm::database::CacheDB::new(EmptyDB::default());
        let user_slot = MipFeeManager::new().user_tokens[caller].slot();
        db.insert_account_storage(MIP_FEE_MANAGER_ADDRESS, user_slot, user_token.into_u256())
            .unwrap();

        let result_token =
            db.get_fee_token(MagnusTxEnv::default(), caller, MagnusHardfork::Genesis)?;
        assert_eq!(result_token, user_token);
        Ok(())
    }

    #[test]
    fn test_get_fee_token_mip20() -> eyre::Result<()> {
        let caller = Address::random();
        let mip20_token = Address::random();

        let tx_env = TxEnv {
            data: Bytes::from_static(b"transfer_data"),
            kind: TxKind::Call(mip20_token),
            caller,
            ..Default::default()
        };
        let tx = MagnusTxEnv {
            inner: tx_env,
            ..Default::default()
        };

        let mut db = EmptyDB::default();
        let result_token = db.get_fee_token(tx, caller, MagnusHardfork::Genesis)?;
        assert_eq!(result_token, DEFAULT_FEE_TOKEN);
        Ok(())
    }

    #[test]
    fn test_get_fee_token_fallback() -> eyre::Result<()> {
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
        let result_token = db.get_fee_token(tx, caller, MagnusHardfork::Genesis)?;
        // Should fallback to DEFAULT_FEE_TOKEN when no preferences are found
        assert_eq!(result_token, DEFAULT_FEE_TOKEN);
        Ok(())
    }

    #[test]
    fn test_get_fee_token_stablecoin_dex() -> eyre::Result<()> {
        let caller = Address::random();
        // Use pathUSD as token_in since it's a known valid USD fee token
        let token_in = DEFAULT_FEE_TOKEN;
        let token_out = address!("0x20C0000000000000000000000000000000000001");

        // Test swapExactAmountIn
        let call = IStablecoinDEX::swapExactAmountInCall {
            tokenIn: token_in,
            tokenOut: token_out,
            amountIn: 1000,
            minAmountOut: 900,
        };

        let tx_env = TxEnv {
            data: call.abi_encode().into(),
            kind: TxKind::Call(STABLECOIN_DEX_ADDRESS),
            caller,
            ..Default::default()
        };
        let tx = MagnusTxEnv {
            inner: tx_env,
            ..Default::default()
        };

        let mut db = EmptyDB::default();
        let token = db.get_fee_token(tx, caller, MagnusHardfork::Genesis)?;
        assert_eq!(token, token_in);

        // Test swapExactAmountOut
        let call = IStablecoinDEX::swapExactAmountOutCall {
            tokenIn: token_in,
            tokenOut: token_out,
            amountOut: 900,
            maxAmountIn: 1000,
        };

        let tx_env = TxEnv {
            data: call.abi_encode().into(),
            kind: TxKind::Call(STABLECOIN_DEX_ADDRESS),
            caller,
            ..Default::default()
        };

        let tx = MagnusTxEnv {
            inner: tx_env,
            ..Default::default()
        };

        let token = db.get_fee_token(tx, caller, MagnusHardfork::Genesis)?;
        assert_eq!(token, token_in);

        Ok(())
    }

    #[test]
    fn test_read_token_balance_typed_storage() -> eyre::Result<()> {
        let token_address = PATH_USD_ADDRESS;
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
    fn test_is_mip20_fee_inference_call() {
        use magnus_precompile_registry::mip20::{IRolesAuth::*, IMIP20::*};

        // Allowed selectors
        assert!(is_mip20_fee_inference_call(&transferCall::SELECTOR));
        assert!(is_mip20_fee_inference_call(&transferWithMemoCall::SELECTOR));
        assert!(is_mip20_fee_inference_call(&distributeRewardCall::SELECTOR));

        // Disallowed selectors
        assert!(!is_mip20_fee_inference_call(&grantRoleCall::SELECTOR));
        assert!(!is_mip20_fee_inference_call(&mintCall::SELECTOR));
        assert!(!is_mip20_fee_inference_call(&approveCall::SELECTOR));

        // Edge cases
        assert!(!is_mip20_fee_inference_call(&[]));
        assert!(!is_mip20_fee_inference_call(&[0x00, 0x01, 0x02]));
    }
}
