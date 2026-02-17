//! Magnus precompile implementations.
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod error;
pub use error::{IntoPrecompileResult, Result};

pub mod storage;

pub mod account_keychain;
pub mod kyc_registry;
pub mod nonce;
pub mod oracle_registry;
pub mod stablecoin_dex;
pub mod mip20;
pub mod mip20_factory;
pub mod mip403_registry;
pub mod mip_fee_manager;
pub mod validator_config;

#[cfg(any(test, feature = "test-utils"))]
pub mod test_util;

use crate::{
    account_keychain::AccountKeychain,
    kyc_registry::KYCRegistry,
    nonce::NonceManager,
    oracle_registry::OracleRegistry,
    stablecoin_dex::StablecoinDEX,
    storage::StorageCtx,
    mip_fee_manager::MipFeeManager,
    mip20::{MIP20Token, is_mip20_prefix},
    mip20_factory::MIP20Factory,
    mip403_registry::MIP403Registry,
    validator_config::ValidatorConfig,
};
use magnus_chainspec::hardfork::MagnusHardfork;

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
    precompile::{PrecompileError, PrecompileId, PrecompileOutput, PrecompileResult},
};

pub use magnus_contracts::precompiles::{
    ACCOUNT_KEYCHAIN_ADDRESS, DEFAULT_FEE_TOKEN, KYC_REGISTRY_ADDRESS,
    NONCE_PRECOMPILE_ADDRESS, ORACLE_REGISTRY_ADDRESS,
    PATH_USD_ADDRESS, STABLECOIN_DEX_ADDRESS, MIP_FEE_MANAGER_ADDRESS, MIP20_FACTORY_ADDRESS,
    MIP403_REGISTRY_ADDRESS, VALIDATOR_CONFIG_ADDRESS,
};

// Re-export storage layout helpers for read-only contexts (e.g., pool validation)
pub use account_keychain::AuthorizedKey;

/// Input per word cost. It covers abi decoding and cloning of input into call data.
///
/// Being careful and pricing it twice as COPY_COST to mitigate different abi decodings.
pub const INPUT_PER_WORD_COST: u64 = 6;

#[inline]
pub fn input_cost(calldata_len: usize) -> u64 {
    let num_words = calldata_len.div_ceil(32) as u64;
    num_words.checked_mul(INPUT_PER_WORD_COST).unwrap_or(u64::MAX)
}

pub trait Precompile {
    fn call(&mut self, calldata: &[u8], msg_sender: Address) -> PrecompileResult;
}

pub fn extend_magnus_precompiles(precompiles: &mut PrecompilesMap, cfg: &CfgEnv<MagnusHardfork>) {
    let chain_id = cfg.chain_id;
    let spec = cfg.spec;
    precompiles.set_precompile_lookup(move |address: &Address| {
        if is_mip20_prefix(*address) {
            Some(MIP20Precompile::create(*address, chain_id, spec))
        } else if *address == MIP20_FACTORY_ADDRESS {
            Some(MIP20FactoryPrecompile::create(chain_id, spec))
        } else if *address == MIP403_REGISTRY_ADDRESS {
            Some(MIP403RegistryPrecompile::create(chain_id, spec))
        } else if *address == MIP_FEE_MANAGER_ADDRESS {
            Some(MipFeeManagerPrecompile::create(chain_id, spec))
        } else if *address == STABLECOIN_DEX_ADDRESS {
            Some(StablecoinDEXPrecompile::create(chain_id, spec))
        } else if *address == NONCE_PRECOMPILE_ADDRESS {
            Some(NoncePrecompile::create(chain_id, spec))
        } else if *address == VALIDATOR_CONFIG_ADDRESS {
            Some(ValidatorConfigPrecompile::create(chain_id, spec))
        } else if *address == ACCOUNT_KEYCHAIN_ADDRESS {
            Some(AccountKeychainPrecompile::create(chain_id, spec))
        } else if *address == ORACLE_REGISTRY_ADDRESS {
            Some(OracleRegistryPrecompile::create(chain_id, spec))
        } else if *address == KYC_REGISTRY_ADDRESS {
            Some(KYCRegistryPrecompile::create(chain_id, spec))
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
    ($id:expr, $chain_id:ident, $spec:ident, |$input:ident| $impl:expr) => {
        DynPrecompile::new_stateful(PrecompileId::Custom($id.into()), move |$input| {
            if !$input.is_direct_call() {
                return Ok(PrecompileOutput::new_reverted(
                    0,
                    DelegateCallNotAllowed {}.abi_encode().into(),
                ));
            }
            let mut storage = crate::storage::evm::EvmPrecompileStorageProvider::new(
                $input.internals,
                $input.gas,
                $chain_id,
                $spec,
                $input.is_static,
            );
            crate::storage::StorageCtx::enter(&mut storage, || {
                $impl.call($input.data, $input.caller)
            })
        })
    };
}

pub struct MipFeeManagerPrecompile;
impl MipFeeManagerPrecompile {
    pub fn create(chain_id: u64, spec: MagnusHardfork) -> DynPrecompile {
        magnus_precompile!("MipFeeManager", chain_id, spec, |input| {
            MipFeeManager::new()
        })
    }
}

pub struct MIP403RegistryPrecompile;
impl MIP403RegistryPrecompile {
    pub fn create(chain_id: u64, spec: MagnusHardfork) -> DynPrecompile {
        magnus_precompile!("MIP403Registry", chain_id, spec, |input| {
            MIP403Registry::new()
        })
    }
}

pub struct MIP20FactoryPrecompile;
impl MIP20FactoryPrecompile {
    pub fn create(chain_id: u64, spec: MagnusHardfork) -> DynPrecompile {
        magnus_precompile!("MIP20Factory", chain_id, spec, |input| {
            MIP20Factory::new()
        })
    }
}

pub struct MIP20Precompile;
impl MIP20Precompile {
    pub fn create(address: Address, chain_id: u64, spec: MagnusHardfork) -> DynPrecompile {
        magnus_precompile!("MIP20Token", chain_id, spec, |input| {
            MIP20Token::from_address(address).expect("MIP20 prefix already verified")
        })
    }
}

pub struct StablecoinDEXPrecompile;
impl StablecoinDEXPrecompile {
    pub fn create(chain_id: u64, spec: MagnusHardfork) -> DynPrecompile {
        magnus_precompile!("StablecoinDEX", chain_id, spec, |input| {
            StablecoinDEX::new()
        })
    }
}

pub struct NoncePrecompile;
impl NoncePrecompile {
    pub fn create(chain_id: u64, spec: MagnusHardfork) -> DynPrecompile {
        magnus_precompile!("NonceManager", chain_id, spec, |input| {
            NonceManager::new()
        })
    }
}

pub struct AccountKeychainPrecompile;
impl AccountKeychainPrecompile {
    pub fn create(chain_id: u64, spec: MagnusHardfork) -> DynPrecompile {
        magnus_precompile!("AccountKeychain", chain_id, spec, |input| {
            AccountKeychain::new()
        })
    }
}

pub struct ValidatorConfigPrecompile;
impl ValidatorConfigPrecompile {
    pub fn create(chain_id: u64, spec: MagnusHardfork) -> DynPrecompile {
        magnus_precompile!("ValidatorConfig", chain_id, spec, |input| {
            ValidatorConfig::new()
        })
    }
}

pub struct OracleRegistryPrecompile;
impl OracleRegistryPrecompile {
    pub fn create(chain_id: u64, spec: MagnusHardfork) -> DynPrecompile {
        magnus_precompile!("OracleRegistry", chain_id, spec, |input| {
            OracleRegistry::new()
        })
    }
}

pub struct KYCRegistryPrecompile;
impl KYCRegistryPrecompile {
    pub fn create(chain_id: u64, spec: MagnusHardfork) -> DynPrecompile {
        magnus_precompile!("KYCRegistry", chain_id, spec, |input| {
            KYCRegistry::new()
        })
    }
}

#[inline]
fn metadata<T: SolCall>(f: impl FnOnce() -> Result<T::Return>) -> PrecompileResult {
    f().into_precompile_result(0, |ret| T::abi_encode_returns(&ret).into())
}

#[inline]
fn view<T: SolCall>(call: T, f: impl FnOnce(T) -> Result<T::Return>) -> PrecompileResult {
    f(call).into_precompile_result(0, |ret| T::abi_encode_returns(&ret).into())
}

#[inline]
fn mutate<T: SolCall>(
    call: T,
    sender: Address,
    f: impl FnOnce(Address, T) -> Result<T::Return>,
) -> PrecompileResult {
    if StorageCtx.is_static() {
        return Ok(PrecompileOutput::new_reverted(
            0,
            StaticCallNotAllowed {}.abi_encode().into(),
        ));
    }
    f(sender, call).into_precompile_result(0, |ret| T::abi_encode_returns(&ret).into())
}

#[inline]
fn mutate_void<T: SolCall>(
    call: T,
    sender: Address,
    f: impl FnOnce(Address, T) -> Result<()>,
) -> PrecompileResult {
    if StorageCtx.is_static() {
        return Ok(PrecompileOutput::new_reverted(
            0,
            StaticCallNotAllowed {}.abi_encode().into(),
        ));
    }
    f(sender, call).into_precompile_result(0, |()| Bytes::new())
}

#[inline]
fn fill_precompile_output(mut output: PrecompileOutput, storage: &StorageCtx) -> PrecompileOutput {
    output.gas_used = storage.gas_used();

    // add refund only if it is not reverted
    if !output.reverted {
        output.gas_refunded = storage.gas_refunded();
    }
    output
}

/// Helper function to return an unknown function selector error.
/// Returns an ABI-encoded UnknownFunctionSelector error with the selector.
#[inline]
pub fn unknown_selector(selector: [u8; 4], gas: u64) -> PrecompileResult {
    error::MagnusPrecompileError::UnknownFunctionSelector(selector).into_precompile_result(gas)
}

/// Helper function to decode calldata and dispatch it.
#[inline]
fn dispatch_call<T>(
    calldata: &[u8],
    decode: impl FnOnce(&[u8]) -> core::result::Result<T, alloy::sol_types::Error>,
    f: impl FnOnce(T) -> PrecompileResult,
) -> PrecompileResult {
    if calldata.len() < 4 {
        return Err(PrecompileError::Other(
            "Invalid input: missing function selector".into(),
        ));
    }

    let storage = StorageCtx::default();
    let result = decode(calldata);

    match result {
        Ok(call) => f(call).map(|res| fill_precompile_output(res, &storage)),
        Err(alloy::sol_types::Error::UnknownSelector { selector, .. }) => {
            unknown_selector(*selector, storage.gas_used())
                .map(|res| fill_precompile_output(res, &storage))
        }
        Err(_) => Ok(fill_precompile_output(
            PrecompileOutput::new_reverted(0, Bytes::new()),
            &storage,
        )),
    }
}

#[cfg(test)]
pub fn expect_precompile_revert<E>(result: &PrecompileResult, expected_error: E)
where
    E: SolInterface + PartialEq + std::fmt::Debug,
{
    match result {
        Ok(result) => {
            assert!(result.reverted);
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
    use crate::mip20::MIP20Token;
    use alloy::primitives::{Address, Bytes, U256, bytes};
    use alloy_evm::{
        EthEvmFactory, EvmEnv, EvmFactory, EvmInternals,
        precompiles::{Precompile as AlloyEvmPrecompile, PrecompileInput},
    };
    use revm::{
        context::ContextTr,
        database::{CacheDB, EmptyDB},
        state::{AccountInfo, Bytecode},
    };
    use magnus_contracts::precompiles::IMIP20;

    #[test]
    fn test_precompile_delegatecall() {
        let (chain_id, spec) = (1, MagnusHardfork::default());
        let precompile = magnus_precompile!("MIP20Token", chain_id, spec, |input| {
            MIP20Token::from_address(PATH_USD_ADDRESS).expect("PATH_USD_ADDRESS is valid")
        });

        let db = CacheDB::new(EmptyDB::new());
        let mut evm = EthEvmFactory::default().create_evm(db, EvmEnv::default());
        let block = evm.block.clone();
        let cfg = evm.cfg.clone();
        let tx = evm.tx.clone();
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
        };

        let result = AlloyEvmPrecompile::call(&precompile, input);

        match result {
            Ok(output) => {
                assert!(output.reverted);
                let decoded = DelegateCallNotAllowed::abi_decode(&output.bytes).unwrap();
                assert!(matches!(decoded, DelegateCallNotAllowed {}));
            }
            Err(_) => panic!("expected reverted output"),
        }
    }

    #[test]
    fn test_precompile_static_call() {
        let (chain_id, spec) = (1, MagnusHardfork::default());
        let precompile = magnus_precompile!("MIP20Token", chain_id, spec, |input| {
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
            let cfg = evm.cfg.clone();
            let tx = evm.tx.clone();
            let evm_internals = EvmInternals::new(evm.journal_mut(), &block, &cfg, &tx);

            let input = PrecompileInput {
                data: &calldata,
                caller: Address::ZERO,
                internals: evm_internals,
                gas: 100_000,
                is_static: true,
                value: U256::ZERO,
                target_address: token_address,
                bytecode_address: token_address,
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
        assert!(output.reverted);
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
        assert!(output.reverted);
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
            !output.reverted,
            "view function should not revert in static context"
        );
    }
}
