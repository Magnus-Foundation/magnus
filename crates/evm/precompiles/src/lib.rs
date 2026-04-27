//! Magnus precompile implementations.
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod error;
pub use error::{IntoPrecompileResult, Result};

pub mod storage;

pub(crate) mod ip_validation;

pub mod account_keychain;
pub mod address_registry;
pub mod nonce;
pub mod signature_verifier;
pub mod stablecoin_dex;
pub mod mip20;
pub mod mip20_factory;
pub mod mip20_issuer_registry;
pub mod mip403_registry;
pub mod mip_fee_manager;
pub mod validator_config;
pub mod validator_config_v2;

#[cfg(any(test, feature = "test-utils"))]
pub mod test_util;

use crate::{
    account_keychain::AccountKeychain, address_registry::AddressRegistry, nonce::NonceManager,
    signature_verifier::SignatureVerifier, stablecoin_dex::StablecoinDEX, storage::StorageCtx,
    mip_fee_manager::MipFeeManager, mip20::MIP20Token, mip20_factory::MIP20Factory,
    mip20_issuer_registry::MIP20IssuerRegistry, mip403_registry::MIP403Registry,
    validator_config::ValidatorConfig, validator_config_v2::ValidatorConfigV2,
};
use magnus_chainspec::hardfork::MagnusHardfork;
use magnus_primitives::MagnusAddressExt;

#[cfg(test)]
use alloy::sol_types::SolInterface;
use alloy::{
    primitives::{Address, Bytes},
    sol,
    sol_types::{SolCall, SolError},
};
use alloy_evm::precompiles::{DynPrecompile, PrecompilesMap};
use revm::{
    context::CfgEnv,
    handler::EthPrecompiles,
    precompile::{PrecompileHalt, PrecompileId, PrecompileOutput, PrecompileResult},
    primitives::hardfork::SpecId,
};

pub use magnus_contracts::precompiles::{
    ACCOUNT_KEYCHAIN_ADDRESS, ADDRESS_REGISTRY_ADDRESS, DEFAULT_FEE_TOKEN,
    NONCE_PRECOMPILE_ADDRESS, PATH_USD_ADDRESS, SIGNATURE_VERIFIER_ADDRESS, STABLECOIN_DEX_ADDRESS,
    TIP_FEE_MANAGER_ADDRESS, MIP20_FACTORY_ADDRESS, MIP20_ISSUER_REGISTRY_ADDRESS,
    MIP403_REGISTRY_ADDRESS, VALIDATOR_CONFIG_ADDRESS, VALIDATOR_CONFIG_V2_ADDRESS,
};

// Re-export storage layout helpers for read-only contexts (e.g., pool validation)
pub use account_keychain::AuthorizedKey;

/// Input per word cost. It covers abi decoding and cloning of input into call data.
///
/// Being careful and pricing it twice as COPY_COST to mitigate different abi decodings.
pub const INPUT_PER_WORD_COST: u64 = 6;

/// Gas cost for `ecrecover` signature verification (used by KeyAuthorization and Permit).
pub const ECRECOVER_GAS: u64 = 3_000;

/// Returns the gas cost for decoding calldata of the given length, rounded up to word boundaries.
#[inline]
pub fn input_cost(calldata_len: usize) -> u64 {
    calldata_len
        .div_ceil(32)
        .saturating_mul(INPUT_PER_WORD_COST as usize) as u64
}

/// Trait implemented by all Magnus precompile contract types.
///
/// Precompiles must provide a dispatcher that decodes the 4-byte function selector from calldata,
/// ABI-decodes the arguments, and routes to the corresponding method.
pub trait Precompile {
    /// Dispatches an EVM call to this precompile.
    ///
    /// Implementations should deduct calldata gas upfront via [`input_cost`], then decode the
    /// 4-byte function selector from `calldata` and route to the matching method using
    /// `dispatch_call` combined with the `view`, `mutate`, or `mutate_void` helpers.
    ///
    /// Business-logic errors are returned as reverted [`PrecompileOutput`]s with ABI-encoded
    /// error data, while fatal failures (e.g. out-of-gas) are returned as [`revm::precompile::PrecompileError`].
    fn call(&mut self, calldata: &[u8], msg_sender: Address) -> PrecompileResult;
}

/// Returns the full Magnus precompiles for the given config.
///
/// Pre-T1C hardforks use Prague precompiles, T1C+ uses Osaka precompiles.
/// Magnus-specific precompiles are also registered via [`extend_magnus_precompiles`].
pub fn magnus_precompiles(cfg: &CfgEnv<MagnusHardfork>) -> PrecompilesMap {
    let spec = if cfg.spec.is_t1c() {
        cfg.spec.into()
    } else {
        SpecId::PRAGUE
    };
    let mut precompiles = PrecompilesMap::from_static(EthPrecompiles::new(spec).precompiles);
    extend_magnus_precompiles(&mut precompiles, cfg);
    precompiles
}

/// Registers Magnus-specific precompiles into an existing [`PrecompilesMap`] by installing a
/// lookup function that matches addresses to their precompile: MIP-20 tokens (by prefix),
/// MIP20Factory, MIP403Registry, MipFeeManager, StablecoinDEX, NonceManager, ValidatorConfig,
/// AccountKeychain, and ValidatorConfigV2. Each precompile is wrapped via the `magnus_precompile!`
/// macro which enforces direct-call-only (no delegatecall) and sets up the storage context.
pub fn extend_magnus_precompiles(precompiles: &mut PrecompilesMap, cfg: &CfgEnv<MagnusHardfork>) {
    let cfg = cfg.clone();

    precompiles.set_precompile_lookup(move |address: &Address| {
        if address.is_tip20() {
            Some(MIP20Token::create_precompile(*address, &cfg))
        } else if *address == MIP20_FACTORY_ADDRESS {
            Some(MIP20Factory::create_precompile(&cfg))
        } else if *address == ADDRESS_REGISTRY_ADDRESS && cfg.spec.is_t3() {
            Some(AddressRegistry::create_precompile(&cfg))
        } else if *address == MIP403_REGISTRY_ADDRESS {
            Some(MIP403Registry::create_precompile(&cfg))
        } else if *address == TIP_FEE_MANAGER_ADDRESS {
            Some(MipFeeManager::create_precompile(&cfg))
        } else if *address == STABLECOIN_DEX_ADDRESS {
            Some(StablecoinDEX::create_precompile(&cfg))
        } else if *address == NONCE_PRECOMPILE_ADDRESS {
            Some(NonceManager::create_precompile(&cfg))
        } else if *address == VALIDATOR_CONFIG_ADDRESS {
            Some(ValidatorConfig::create_precompile(&cfg))
        } else if *address == ACCOUNT_KEYCHAIN_ADDRESS {
            Some(AccountKeychain::create_precompile(&cfg))
        } else if *address == VALIDATOR_CONFIG_V2_ADDRESS {
            Some(ValidatorConfigV2::create_precompile(&cfg))
        } else if *address == SIGNATURE_VERIFIER_ADDRESS && cfg.spec.is_t3() {
            Some(SignatureVerifier::create_precompile(&cfg))
        } else if *address == MIP20_ISSUER_REGISTRY_ADDRESS && cfg.spec.is_t4() {
            // T4 hardfork: multi-currency fees + issuer-allowlist gate
            // (multi-currency-fees-design.md §4, v3.8.2). Stub implementation in G0;
            // governance-gated allowlist logic lands in G4.
            Some(MIP20IssuerRegistry::create_precompile(&cfg))
        } else {
            None
        }
    });
}

sol! {
    error DelegateCallNotAllowed();
    error StaticCallNotAllowed();
}

macro_rules! magnus_precompile {
    ($id:expr, $cfg:expr, |$input:ident| $impl:expr) => {{
        let spec = $cfg.spec;
        let gas_params = $cfg.gas_params.clone();
        DynPrecompile::new_stateful(PrecompileId::Custom($id.into()), move |$input| {
            if !$input.is_direct_call() {
                return Ok(PrecompileOutput::revert(
                    0,
                    DelegateCallNotAllowed {}.abi_encode().into(),
                    $input.reservoir,
                ));
            }
            let mut storage = crate::storage::evm::EvmPrecompileStorageProvider::new(
                $input.internals,
                $input.gas,
                $input.reservoir,
                spec,
                $input.is_static,
                gas_params.clone(),
            );
            crate::storage::StorageCtx::enter(&mut storage, || {
                $impl.call($input.data, $input.caller)
            })
        })
    }};
}

impl MipFeeManager {
    /// Creates the EVM precompile for this type.
    pub fn create_precompile(cfg: &CfgEnv<MagnusHardfork>) -> DynPrecompile {
        magnus_precompile!("MipFeeManager", cfg, |input| { Self::new() })
    }
}

impl AddressRegistry {
    /// Creates the EVM precompile for this type.
    pub fn create_precompile(cfg: &CfgEnv<MagnusHardfork>) -> DynPrecompile {
        magnus_precompile!("AddressRegistry", cfg, |input| { Self::new() })
    }
}

impl MIP403Registry {
    /// Creates the EVM precompile for this type.
    pub fn create_precompile(cfg: &CfgEnv<MagnusHardfork>) -> DynPrecompile {
        magnus_precompile!("MIP403Registry", cfg, |input| { Self::new() })
    }
}

impl MIP20Factory {
    /// Creates the EVM precompile for this type.
    pub fn create_precompile(cfg: &CfgEnv<MagnusHardfork>) -> DynPrecompile {
        magnus_precompile!("MIP20Factory", cfg, |input| { Self::new() })
    }
}

impl MIP20IssuerRegistry {
    /// Creates the EVM precompile for this type.
    pub fn create_precompile(cfg: &CfgEnv<MagnusHardfork>) -> DynPrecompile {
        magnus_precompile!("MIP20IssuerRegistry", cfg, |input| { Self::new() })
    }
}

impl MIP20Token {
    /// Creates the EVM precompile for this type.
    pub fn create_precompile(address: Address, cfg: &CfgEnv<MagnusHardfork>) -> DynPrecompile {
        magnus_precompile!("MIP20Token", cfg, |input| {
            Self::from_address(address).expect("MIP20 prefix already verified")
        })
    }
}

impl StablecoinDEX {
    /// Creates the EVM precompile for this type.
    pub fn create_precompile(cfg: &CfgEnv<MagnusHardfork>) -> DynPrecompile {
        magnus_precompile!("StablecoinDEX", cfg, |input| { Self::new() })
    }
}

impl NonceManager {
    /// Creates the EVM precompile for this type.
    pub fn create_precompile(cfg: &CfgEnv<MagnusHardfork>) -> DynPrecompile {
        magnus_precompile!("NonceManager", cfg, |input| { Self::new() })
    }
}

impl AccountKeychain {
    /// Creates the EVM precompile for this type.
    pub fn create_precompile(cfg: &CfgEnv<MagnusHardfork>) -> DynPrecompile {
        magnus_precompile!("AccountKeychain", cfg, |input| { Self::new() })
    }
}

impl ValidatorConfig {
    /// Creates the EVM precompile for this type.
    pub fn create_precompile(cfg: &CfgEnv<MagnusHardfork>) -> DynPrecompile {
        magnus_precompile!("ValidatorConfig", cfg, |input| { Self::new() })
    }
}

impl ValidatorConfigV2 {
    /// Creates the EVM precompile for this type.
    pub fn create_precompile(cfg: &CfgEnv<MagnusHardfork>) -> DynPrecompile {
        magnus_precompile!("ValidatorConfigV2", cfg, |input| { Self::new() })
    }
}

impl SignatureVerifier {
    /// Creates the EVM precompile for this type.
    pub fn create_precompile(cfg: &CfgEnv<MagnusHardfork>) -> DynPrecompile {
        magnus_precompile!("SignatureVerifier", cfg, |input| { Self::new() })
    }
}

/// Dispatches a parameterless view call, encoding the return via `T`.
#[inline]
fn metadata<T: SolCall>(f: impl FnOnce() -> Result<T::Return>) -> PrecompileResult {
    f().into_precompile_result(0, 0, |ret| T::abi_encode_returns(&ret).into())
}

/// Dispatches a read-only call with decoded arguments, encoding the return via `T`.
#[inline]
fn view<T: SolCall>(call: T, f: impl FnOnce(T) -> Result<T::Return>) -> PrecompileResult {
    f(call).into_precompile_result(0, 0, |ret| T::abi_encode_returns(&ret).into())
}

/// Dispatches a state-mutating call that returns ABI-encoded data.
///
/// Rejects static calls with [`StaticCallNotAllowed`].
#[inline]
fn mutate<T: SolCall>(
    call: T,
    sender: Address,
    f: impl FnOnce(Address, T) -> Result<T::Return>,
) -> PrecompileResult {
    if StorageCtx.is_static() {
        return Ok(PrecompileOutput::revert(
            0,
            StaticCallNotAllowed {}.abi_encode().into(),
            StorageCtx.reservoir(),
        ));
    }
    f(sender, call).into_precompile_result(0, 0, |ret| T::abi_encode_returns(&ret).into())
}

/// Dispatches a state-mutating call that returns no data (e.g. `approve`, `transfer`).
///
/// Rejects static calls with [`StaticCallNotAllowed`].
#[inline]
fn mutate_void<T: SolCall>(
    call: T,
    sender: Address,
    f: impl FnOnce(Address, T) -> Result<()>,
) -> PrecompileResult {
    if StorageCtx.is_static() {
        return Ok(PrecompileOutput::revert(
            0,
            StaticCallNotAllowed {}.abi_encode().into(),
            StorageCtx.reservoir(),
        ));
    }
    f(sender, call).into_precompile_result(0, 0, |()| Bytes::new())
}

/// Deducts the calldata input cost, returning an OOG halt result if insufficient gas.
#[inline]
pub(crate) fn charge_input_cost(
    storage: &mut StorageCtx,
    calldata: &[u8],
) -> Option<PrecompileResult> {
    if storage.deduct_gas(input_cost(calldata.len())).is_err() {
        return Some(Ok(storage.halt_output(PrecompileHalt::OutOfGas)));
    }
    None
}

/// A selector schedule at a given hardfork boundary.
///
/// Before the hardfork activates, selectors in `added` are treated as unknown.
/// After it activates, selectors in `dropped` are treated as unknown.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SelectorSchedule<'a> {
    hardfork: MagnusHardfork,
    added: &'a [[u8; 4]],
    dropped: &'a [[u8; 4]],
}

impl<'a> SelectorSchedule<'a> {
    /// Creates a new schedule anchored at `hardfork` with no selectors registered yet.
    pub(crate) const fn new(hardfork: MagnusHardfork) -> Self {
        Self {
            hardfork,
            added: &[],
            dropped: &[],
        }
    }

    /// Registers selectors that are introduced at this hardfork boundary.
    ///
    /// These selectors are treated as unknown BEFORE `hardfork` activates.
    pub(crate) const fn with_added(mut self, selectors: &'a [[u8; 4]]) -> Self {
        self.added = selectors;
        self
    }

    /// Registers selectors that are removed at this hardfork boundary.
    ///
    /// These selectors are treated as unknown ONCE `hardfork` activates.
    pub(crate) const fn with_dropped(mut self, selectors: &'a [[u8; 4]]) -> Self {
        self.dropped = selectors;
        self
    }

    /// Returns `true` if this schedule gates out `selector` under the `active` hardfork.
    #[inline]
    fn rejects(self, selector: [u8; 4], active: MagnusHardfork) -> bool {
        if self.hardfork <= active {
            self.dropped
        } else {
            self.added
        }
        .contains(&selector)
    }
}

/// Applies hardfork selector schedules, decodes calldata via `decode`, then dispatches to `f`.
///
/// Handles missing selectors (revert on T1+, error on earlier forks), hardfork-gated selectors,
/// unknown selectors (ABI-encoded `UnknownFunctionSelector`), and malformed ABI data (empty
/// revert).
#[inline]
pub(crate) fn dispatch_call<T>(
    calldata: &[u8],
    hardforks: &[SelectorSchedule<'_>],
    decode: impl FnOnce(&[u8]) -> core::result::Result<T, alloy::sol_types::Error>,
    f: impl FnOnce(T) -> PrecompileResult,
) -> PrecompileResult {
    let storage = StorageCtx::default();

    if calldata.len() < 4 {
        if storage.spec().is_t1() {
            return Ok(storage.revert_output(Bytes::new()));
        } else {
            return Ok(storage.halt_output(PrecompileHalt::Other(
                "Invalid input: missing function selector".into(),
            )));
        }
    }

    let selector: [u8; 4] = calldata[..4].try_into().expect("calldata len >= 4");
    if hardforks
        .iter()
        .any(|schedule| schedule.rejects(selector, storage.spec()))
    {
        return storage.error_result(error::MagnusPrecompileError::UnknownFunctionSelector(
            selector,
        ));
    }

    let result = decode(calldata);

    match result {
        Ok(call) => f(call).map(|mut res| {
            // TODO: fix this, each precompile handler should either return output with proper gas values or don't return any gas values at all.
            res.gas_used = storage.gas_used();
            res.reservoir = storage.reservoir();
            res
        }),
        Err(alloy::sol_types::Error::UnknownSelector { selector, .. }) => storage.error_result(
            error::MagnusPrecompileError::UnknownFunctionSelector(*selector),
        ),
        Err(_) => Ok(storage.revert_output(Bytes::new())),
    }
}

/// Asserts that `result` is a reverted output whose bytes decode to `expected_error`.
#[cfg(test)]
pub fn expect_precompile_revert<E>(result: &PrecompileResult, expected_error: E)
where
    E: SolInterface + PartialEq + std::fmt::Debug,
{
    match result {
        Ok(result) => {
            assert!(result.is_revert());
            let decoded = E::abi_decode(&result.bytes).unwrap();
            assert_eq!(decoded, expected_error);
        }
        Err(other) => {
            panic!("expected reverted output, got: {other:?}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        storage::{StorageCtx, hashmap::HashMapStorageProvider},
        mip20::MIP20Token,
    };
    use alloy::primitives::{Address, Bytes, U256, bytes};
    use alloy_evm::{
        EthEvmFactory, EvmEnv, EvmFactory, EvmInternals,
        precompiles::{Precompile as AlloyEvmPrecompile, PrecompileInput},
    };
    use revm::{
        context::{ContextTr, TxEnv},
        database::{CacheDB, EmptyDB},
        state::{AccountInfo, Bytecode},
    };
    use magnus_contracts::precompiles::{IMIP20, UnknownFunctionSelector};

    #[test]
    fn test_precompile_delegatecall() {
        let cfg = CfgEnv::<MagnusHardfork>::default();
        let precompile = magnus_precompile!("MIP20Token", &cfg, |input| {
            MIP20Token::from_address(PATH_USD_ADDRESS).expect("PATH_USD_ADDRESS is valid")
        });

        let db = CacheDB::new(EmptyDB::new());
        let mut evm = EthEvmFactory::default().create_evm(db, EvmEnv::default());
        let block = evm.block.clone();
        let tx = TxEnv::default();
        let evm_internals = EvmInternals::new(evm.journal_mut(), &block, &cfg, &tx);

        let target_address = Address::random();
        let bytecode_address = Address::random();
        let input = PrecompileInput {
            data: &Bytes::new(),
            caller: Address::ZERO,
            internals: evm_internals,
            gas: 0,
            value: U256::ZERO,
            is_static: false,
            target_address,
            bytecode_address,
            reservoir: 0,
        };

        let result = AlloyEvmPrecompile::call(&precompile, input);

        match result {
            Ok(output) => {
                assert!(output.is_revert());
                let decoded = DelegateCallNotAllowed::abi_decode(&output.bytes).unwrap();
                assert!(matches!(decoded, DelegateCallNotAllowed {}));
            }
            Err(_) => panic!("expected reverted output"),
        }
    }

    #[test]
    fn test_precompile_static_call() {
        let cfg = CfgEnv::<MagnusHardfork>::default();
        let tx = TxEnv::default();
        let precompile = magnus_precompile!("MIP20Token", &cfg, |input| {
            MIP20Token::from_address(PATH_USD_ADDRESS).expect("PATH_USD_ADDRESS is valid")
        });

        let token_address = PATH_USD_ADDRESS;

        let call_static = |calldata: Bytes| {
            let mut db = CacheDB::new(EmptyDB::new());
            db.insert_account_info(
                token_address,
                AccountInfo {
                    code: Some(Bytecode::new_raw(bytes!("0xEF"))),
                    ..Default::default()
                },
            );
            let mut evm = EthEvmFactory::default().create_evm(db, EvmEnv::default());
            let block = evm.block.clone();
            let evm_internals = EvmInternals::new(evm.journal_mut(), &block, &cfg, &tx);

            let input = PrecompileInput {
                data: &calldata,
                caller: Address::ZERO,
                internals: evm_internals,
                gas: 1_000_000,
                is_static: true,
                value: U256::ZERO,
                target_address: token_address,
                bytecode_address: token_address,
                reservoir: 0,
            };

            AlloyEvmPrecompile::call(&precompile, input)
        };

        // Static calls into mutating functions should fail
        let result = call_static(Bytes::from(
            IMIP20::transferCall {
                to: Address::random(),
                amount: U256::from(100),
            }
            .abi_encode(),
        ));
        let output = result.expect("expected Ok");
        assert!(output.is_revert());
        assert!(StaticCallNotAllowed::abi_decode(&output.bytes).is_ok());

        // Static calls into mutate void functions should fail
        let result = call_static(Bytes::from(
            IMIP20::approveCall {
                spender: Address::random(),
                amount: U256::from(100),
            }
            .abi_encode(),
        ));
        let output = result.expect("expected Ok");
        assert!(output.is_revert());
        assert!(StaticCallNotAllowed::abi_decode(&output.bytes).is_ok());

        // Static calls into view functions should succeed
        let result = call_static(Bytes::from(
            IMIP20::balanceOfCall {
                account: Address::random(),
            }
            .abi_encode(),
        ));
        let output = result.expect("expected Ok");
        assert!(
            !output.is_revert(),
            "view function should not revert in static context"
        );
    }

    #[test]
    fn test_invalid_calldata_hardfork_behavior() {
        let call_with_spec = |calldata: Bytes, spec: MagnusHardfork| {
            let mut cfg = CfgEnv::<MagnusHardfork>::default();
            cfg.set_spec_and_mainnet_gas_params(spec);
            let tx = TxEnv::default();
            let precompile = magnus_precompile!("MIP20Token", &cfg, |input| {
                MIP20Token::from_address(PATH_USD_ADDRESS).expect("PATH_USD_ADDRESS is valid")
            });

            let mut db = CacheDB::new(EmptyDB::new());
            db.insert_account_info(
                PATH_USD_ADDRESS,
                AccountInfo {
                    code: Some(Bytecode::new_raw(bytes!("0xEF"))),
                    ..Default::default()
                },
            );
            let mut evm = EthEvmFactory::default().create_evm(db, EvmEnv::default());
            let block = evm.block.clone();
            let evm_internals = EvmInternals::new(evm.journal_mut(), &block, &cfg, &tx);

            let input = PrecompileInput {
                data: &calldata,
                caller: Address::ZERO,
                internals: evm_internals,
                gas: 1_000_000,
                is_static: false,
                value: U256::ZERO,
                target_address: PATH_USD_ADDRESS,
                bytecode_address: PATH_USD_ADDRESS,
                reservoir: 0,
            };

            AlloyEvmPrecompile::call(&precompile, input)
        };

        // T1: empty calldata (missing selector) should return a reverted output
        let empty = call_with_spec(Bytes::new(), MagnusHardfork::T1)
            .expect("T1: expected Ok with reverted output");
        assert!(empty.is_revert(), "T1: expected reverted output");
        assert!(empty.bytes.is_empty());
        assert!(empty.gas_used != 0);

        // T1: unknown selector should return a reverted output with UnknownFunctionSelector error
        let unknown = call_with_spec(Bytes::from([0xAA; 4]), MagnusHardfork::T1)
            .expect("T1: expected Ok with reverted output");
        assert!(unknown.is_revert(), "T1: expected reverted output");

        // Verify it's an UnknownFunctionSelector error with the correct selector
        let decoded =
            magnus_contracts::precompiles::UnknownFunctionSelector::abi_decode(&unknown.bytes)
                .expect("T1: expected UnknownFunctionSelector error");
        assert_eq!(decoded.selector.as_slice(), &[0xAA, 0xAA, 0xAA, 0xAA]);

        // Verify gas is tracked for both cases (unknown selector may cost slightly more due `INPUT_PER_WORD_COST`)
        assert!(unknown.gas_used >= empty.gas_used);

        // Pre-T1 (T0): invalid calldata should return a halted output
        let result = call_with_spec(Bytes::new(), MagnusHardfork::T0);
        let output = result.expect("T0: expected Ok(halt) for invalid calldata");
        assert!(
            output.is_halt(),
            "T0: expected halted output for invalid calldata"
        );
    }

    #[test]
    fn test_dispatch_call_applies_hardfork_selector_gates() -> eyre::Result<()> {
        alloy::sol! {
            interface ISelectorGatedTest {
                function stable() external;
                function t2Added(uint256 value) external;
                function t3Removed() external;
            }
        }

        const SELECTOR_SCHEDULE: &[SelectorSchedule<'static>] = &[
            SelectorSchedule::new(MagnusHardfork::T2)
                .with_added(&[ISelectorGatedTest::t2AddedCall::SELECTOR]),
            SelectorSchedule::new(MagnusHardfork::T3)
                .with_dropped(&[ISelectorGatedTest::t3RemovedCall::SELECTOR]),
        ];

        let call_with_spec = |spec: MagnusHardfork, calldata: &[u8]| {
            let mut storage = HashMapStorageProvider::new_with_spec(1, spec);
            StorageCtx::enter(&mut storage, || {
                dispatch_call(
                    calldata,
                    SELECTOR_SCHEDULE,
                    ISelectorGatedTest::ISelectorGatedTestCalls::abi_decode,
                    |call| match call {
                        ISelectorGatedTest::ISelectorGatedTestCalls::stable(_) => {
                            Ok(PrecompileOutput::new(0, Bytes::from_static(b"stable"), 0))
                        }
                        ISelectorGatedTest::ISelectorGatedTestCalls::t2Added(_) => {
                            Ok(PrecompileOutput::new(0, Bytes::from_static(b"added"), 0))
                        }
                        ISelectorGatedTest::ISelectorGatedTestCalls::t3Removed(_) => {
                            Ok(PrecompileOutput::new(0, Bytes::from_static(b"removed"), 0))
                        }
                    },
                )
            })
        };

        let t2_added_calldata = ISelectorGatedTest::t2AddedCall { value: U256::ZERO }.abi_encode();
        let t3_removed_calldata = ISelectorGatedTest::t3RemovedCall {}.abi_encode();

        // pre-T2: selectors introduced at T2 must still look unknown.
        let pre_t2_added = call_with_spec(MagnusHardfork::T1, &t2_added_calldata)?;
        assert!(pre_t2_added.is_revert());
        let decoded = UnknownFunctionSelector::abi_decode(&pre_t2_added.bytes)?;
        assert_eq!(
            decoded.selector.as_slice(),
            &ISelectorGatedTest::t2AddedCall::SELECTOR
        );

        // T2+: that selector becomes available and dispatches normally.
        let post_t2_added = call_with_spec(MagnusHardfork::T2, &t2_added_calldata)?;
        assert!(!post_t2_added.is_revert());
        assert_eq!(post_t2_added.bytes.as_ref(), b"added");

        // pre-T3: selectors removed at T3 still dispatch normally.
        let pre_t3_removed = call_with_spec(MagnusHardfork::T2, &t3_removed_calldata)?;
        assert!(!pre_t3_removed.is_revert());
        assert_eq!(pre_t3_removed.bytes.as_ref(), b"removed");

        // T3+: the removed selector must now revert as unknown.
        let post_t3_removed = call_with_spec(MagnusHardfork::T3, &t3_removed_calldata)?;
        assert!(post_t3_removed.is_revert());
        let decoded = UnknownFunctionSelector::abi_decode(&post_t3_removed.bytes)?;
        assert_eq!(
            decoded.selector.as_slice(),
            &ISelectorGatedTest::t3RemovedCall::SELECTOR
        );

        // preT2: gated selectors must return `UnknownFunctionSelector` even for selector-only calldata.
        let malformed_added = call_with_spec(
            MagnusHardfork::T1,
            &ISelectorGatedTest::t2AddedCall::SELECTOR,
        )?;
        assert!(malformed_added.is_revert());
        let decoded = UnknownFunctionSelector::abi_decode(&malformed_added.bytes)?;
        assert_eq!(
            decoded.selector.as_slice(),
            &ISelectorGatedTest::t2AddedCall::SELECTOR
        );

        Ok(())
    }

    #[test]
    fn test_input_cost_returns_non_zero_for_input() {
        // Empty input should cost 0
        assert_eq!(input_cost(0), 0);

        // 1 byte should cost INPUT_PER_WORD_COST (rounds up to 1 word)
        assert_eq!(input_cost(1), INPUT_PER_WORD_COST);

        // 32 bytes (1 word) should cost INPUT_PER_WORD_COST
        assert_eq!(input_cost(32), INPUT_PER_WORD_COST);

        // 33 bytes (2 words) should cost 2 * INPUT_PER_WORD_COST
        assert_eq!(input_cost(33), INPUT_PER_WORD_COST * 2);
    }

    #[test]
    fn test_extend_magnus_precompiles_registers_precompiles() {
        let mut cfg = CfgEnv::<MagnusHardfork>::default();
        cfg.set_spec_and_mainnet_gas_params(MagnusHardfork::T3);
        let precompiles = magnus_precompiles(&cfg);

        // MIP20Factory should be registered
        let factory_precompile = precompiles.get(&MIP20_FACTORY_ADDRESS);
        assert!(
            factory_precompile.is_some(),
            "MIP20Factory should be registered"
        );

        // MIP403Registry should be registered
        let registry_precompile = precompiles.get(&MIP403_REGISTRY_ADDRESS);
        assert!(
            registry_precompile.is_some(),
            "MIP403Registry should be registered"
        );

        // MipFeeManager should be registered
        let fee_manager_precompile = precompiles.get(&TIP_FEE_MANAGER_ADDRESS);
        assert!(
            fee_manager_precompile.is_some(),
            "MipFeeManager should be registered"
        );

        // StablecoinDEX should be registered
        let dex_precompile = precompiles.get(&STABLECOIN_DEX_ADDRESS);
        assert!(
            dex_precompile.is_some(),
            "StablecoinDEX should be registered"
        );

        // NonceManager should be registered
        let nonce_precompile = precompiles.get(&NONCE_PRECOMPILE_ADDRESS);
        assert!(
            nonce_precompile.is_some(),
            "NonceManager should be registered"
        );

        // ValidatorConfig should be registered
        let validator_precompile = precompiles.get(&VALIDATOR_CONFIG_ADDRESS);
        assert!(
            validator_precompile.is_some(),
            "ValidatorConfig should be registered"
        );

        // ValidatorConfigV2 should be registered
        let validator_v2_precompile = precompiles.get(&VALIDATOR_CONFIG_V2_ADDRESS);
        assert!(
            validator_v2_precompile.is_some(),
            "ValidatorConfigV2 should be registered"
        );

        // AccountKeychain should be registered
        let keychain_precompile = precompiles.get(&ACCOUNT_KEYCHAIN_ADDRESS);
        assert!(
            keychain_precompile.is_some(),
            "AccountKeychain should be registered"
        );

        // SignatureVerifier should be registered at T3
        let sig_verifier_precompile = precompiles.get(&SIGNATURE_VERIFIER_ADDRESS);
        assert!(
            sig_verifier_precompile.is_some(),
            "SignatureVerifier should be registered at T3"
        );

        // MIP20 tokens with prefix should be registered
        let mip20_precompile = precompiles.get(&PATH_USD_ADDRESS);
        assert!(
            mip20_precompile.is_some(),
            "MIP20 tokens should be registered"
        );

        // Random address without MIP20 prefix should NOT be registered
        let random_address = Address::random();
        let random_precompile = precompiles.get(&random_address);
        assert!(
            random_precompile.is_none(),
            "Random address should not be a precompile"
        );
    }

    #[test]
    fn test_signature_verifier_not_registered_pre_t3() {
        let cfg = CfgEnv::<MagnusHardfork>::default();
        let precompiles = magnus_precompiles(&cfg);

        assert!(
            precompiles.get(&SIGNATURE_VERIFIER_ADDRESS).is_none(),
            "SignatureVerifier should NOT be registered before T3"
        );
    }

    /// MIP20IssuerRegistry is the new precompile introduced in T4 hardfork
    /// (multi-currency-fees-design.md §4, v3.8.2). At every hardfork before
    /// T4 it must be unresolvable; at T4 it must resolve.
    #[test]
    fn test_issuer_registry_not_registered_pre_t4() {
        for spec in [
            MagnusHardfork::Genesis,
            MagnusHardfork::T0,
            MagnusHardfork::T1,
            MagnusHardfork::T1A,
            MagnusHardfork::T1B,
            MagnusHardfork::T1C,
            MagnusHardfork::T2,
            MagnusHardfork::T3,
        ] {
            let mut cfg = CfgEnv::<MagnusHardfork>::default();
            cfg.set_spec_and_mainnet_gas_params(spec);
            let precompiles = magnus_precompiles(&cfg);

            assert!(
                precompiles.get(&MIP20_ISSUER_REGISTRY_ADDRESS).is_none(),
                "MIP20IssuerRegistry must NOT be registered at {spec:?} (pre-T4)"
            );
        }
    }

    #[test]
    fn test_issuer_registry_registered_at_t4() {
        let mut cfg = CfgEnv::<MagnusHardfork>::default();
        cfg.set_spec_and_mainnet_gas_params(MagnusHardfork::T4);
        let precompiles = magnus_precompiles(&cfg);

        let registry_precompile = precompiles.get(&MIP20_ISSUER_REGISTRY_ADDRESS);
        assert!(
            registry_precompile.is_some(),
            "MIP20IssuerRegistry MUST be registered at T4"
        );
    }

    /// Verifies the issuer-registry address sits adjacent to the factory
    /// (per design doc §2.2 architecture diagram) and does not collide with
    /// any other Magnus-allocated precompile address constant.
    #[test]
    fn test_issuer_registry_address_does_not_collide() {
        let other_addresses = [
            TIP_FEE_MANAGER_ADDRESS,
            PATH_USD_ADDRESS,
            MIP403_REGISTRY_ADDRESS,
            MIP20_FACTORY_ADDRESS,
            STABLECOIN_DEX_ADDRESS,
            NONCE_PRECOMPILE_ADDRESS,
            VALIDATOR_CONFIG_ADDRESS,
            ACCOUNT_KEYCHAIN_ADDRESS,
            VALIDATOR_CONFIG_V2_ADDRESS,
            ADDRESS_REGISTRY_ADDRESS,
            SIGNATURE_VERIFIER_ADDRESS,
        ];

        for addr in other_addresses {
            assert_ne!(
                addr, MIP20_ISSUER_REGISTRY_ADDRESS,
                "MIP20_ISSUER_REGISTRY_ADDRESS must not collide with {addr}"
            );
        }
    }

    #[test]
    fn test_p256verify_availability_across_t1c_boundary() {
        let has_p256 = |spec: MagnusHardfork| -> bool {
            // P256VERIFY lives at address 0x100 (256), added in Osaka
            let p256_addr = Address::from_word(U256::from(256).into());

            let mut cfg = CfgEnv::<MagnusHardfork>::default();
            cfg.set_spec_and_mainnet_gas_params(spec);
            magnus_precompiles(&cfg).get(&p256_addr).is_some()
        };

        // Pre-T1C hardforks should use Prague precompiles (no P256VERIFY)
        for spec in [
            MagnusHardfork::Genesis,
            MagnusHardfork::T0,
            MagnusHardfork::T1,
            MagnusHardfork::T1A,
            MagnusHardfork::T1B,
        ] {
            assert!(
                !has_p256(spec),
                "P256VERIFY should NOT be available at {spec:?} (pre-T1C)"
            );
        }

        // T1C+ hardforks should use Osaka precompiles (P256VERIFY available)
        for spec in [MagnusHardfork::T1C, MagnusHardfork::T2] {
            assert!(
                has_p256(spec),
                "P256VERIFY should be available at {spec:?} (T1C+)"
            );
        }
    }
}
