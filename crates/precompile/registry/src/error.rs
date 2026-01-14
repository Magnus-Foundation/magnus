use std::{
    collections::HashMap,
    sync::{Arc, LazyLock},
};

use crate::mip20::MIP20Error;
use alloy::{
    primitives::{Selector, U256},
    sol_types::{Panic, PanicKind, SolError, SolInterface},
};
use revm::precompile::{PrecompileError, PrecompileOutput, PrecompileResult};
use magnus_contracts::precompiles::{
    AccountKeychainError, FeeManagerError, NonceError, RolesAuthError, StablecoinDEXError,
    MIP20FactoryError, MIP403RegistryError, MIPFeeAMMError, UnknownFunctionSelector,
    ValidatorConfigError,
};

/// Top-level error type for all Magnus precompile operations
#[derive(
    Debug, Clone, PartialEq, Eq, thiserror::Error, derive_more::From, derive_more::TryInto,
)]
pub enum MagnusPrecompileError {
    /// Stablecoin DEX error
    #[error("Stablecoin DEX error: {0:?}")]
    StablecoinDEX(StablecoinDEXError),

    /// Error from MIP20 token
    #[error("MIP20 token error: {0:?}")]
    MIP20(MIP20Error),

    /// Error from MIP20 factory
    #[error("MIP20 factory error: {0:?}")]
    MIP20Factory(MIP20FactoryError),

    /// Error from roles auth
    #[error("Roles auth error: {0:?}")]
    RolesAuthError(RolesAuthError),

    /// Error from 403 registry
    #[error("MIP403 registry error: {0:?}")]
    MIP403RegistryError(MIP403RegistryError),

    /// Error from MIP fee manager
    #[error("MIP fee manager error: {0:?}")]
    FeeManagerError(FeeManagerError),

    /// Error from MIP fee AMM
    #[error("MIP fee AMM error: {0:?}")]
    MIPFeeAMMError(MIPFeeAMMError),

    /// Error from Magnus Transaction nonce manager
    #[error("Magnus Transaction nonce error: {0:?}")]
    NonceError(NonceError),

    #[error("Panic({0:?})")]
    Panic(PanicKind),

    /// Error from validator config
    #[error("Validator config error: {0:?}")]
    ValidatorConfigError(ValidatorConfigError),

    /// Error from account keychain precompile
    #[error("Account keychain error: {0:?}")]
    AccountKeychainError(AccountKeychainError),

    #[error("Gas limit exceeded")]
    OutOfGas,

    #[error("Unknown function selector: {0:?}")]
    UnknownFunctionSelector([u8; 4]),

    #[error("Fatal precompile error: {0:?}")]
    #[from(skip)]
    Fatal(String),
}

/// Result type alias for Magnus precompile operations
pub type Result<T> = std::result::Result<T, MagnusPrecompileError>;

impl MagnusPrecompileError {
    pub fn under_overflow() -> Self {
        Self::Panic(PanicKind::UnderOverflow)
    }

    pub fn array_oob() -> Self {
        Self::Panic(PanicKind::ArrayOutOfBounds)
    }

    pub fn into_precompile_result(self, gas: u64) -> PrecompileResult {
        let bytes = match self {
            Self::StablecoinDEX(e) => e.abi_encode().into(),
            Self::MIP20(e) => e.abi_encode().into(),
            Self::MIP20Factory(e) => e.abi_encode().into(),
            Self::RolesAuthError(e) => e.abi_encode().into(),
            Self::MIP403RegistryError(e) => e.abi_encode().into(),
            Self::FeeManagerError(e) => e.abi_encode().into(),
            Self::MIPFeeAMMError(e) => e.abi_encode().into(),
            Self::NonceError(e) => e.abi_encode().into(),
            Self::Panic(kind) => {
                let panic = Panic {
                    code: U256::from(kind as u32),
                };

                panic.abi_encode().into()
            }
            Self::ValidatorConfigError(e) => e.abi_encode().into(),
            Self::AccountKeychainError(e) => e.abi_encode().into(),
            Self::OutOfGas => {
                return Err(PrecompileError::OutOfGas);
            }
            Self::UnknownFunctionSelector(selector) => UnknownFunctionSelector {
                selector: selector.into(),
            }
            .abi_encode()
            .into(),
            Self::Fatal(msg) => {
                return Err(PrecompileError::Fatal(msg));
            }
        };
        Ok(PrecompileOutput::new_reverted(gas, bytes))
    }
}

pub fn add_errors_to_registry<T: SolInterface>(
    registry: &mut MagnusPrecompileErrorRegistry,
    converter: impl Fn(T) -> MagnusPrecompileError + 'static + Send + Sync,
) {
    let converter = Arc::new(converter);
    for selector in T::selectors() {
        let converter = Arc::clone(&converter);
        registry.insert(
            selector.into(),
            Box::new(move |data: &[u8]| {
                T::abi_decode(data)
                    .ok()
                    .map(|error| DecodedMagnusPrecompileError {
                        error: converter(error),
                        revert_bytes: data,
                    })
            }),
        );
    }
}

pub struct DecodedMagnusPrecompileError<'a> {
    pub error: MagnusPrecompileError,
    pub revert_bytes: &'a [u8],
}

pub type MagnusPrecompileErrorRegistry = HashMap<
    Selector,
    Box<dyn for<'a> Fn(&'a [u8]) -> Option<DecodedMagnusPrecompileError<'a>> + Send + Sync>,
>;

/// Returns a HashMap mapping error selectors to their decoder functions
/// The decoder returns a `MagnusPrecompileError` variant for the decoded error
pub fn error_decoder_registry() -> MagnusPrecompileErrorRegistry {
    let mut registry: MagnusPrecompileErrorRegistry = HashMap::new();

    add_errors_to_registry(&mut registry, MagnusPrecompileError::StablecoinDEX);
    add_errors_to_registry(&mut registry, MagnusPrecompileError::MIP20);
    add_errors_to_registry(&mut registry, MagnusPrecompileError::MIP20Factory);
    add_errors_to_registry(&mut registry, MagnusPrecompileError::RolesAuthError);
    add_errors_to_registry(&mut registry, MagnusPrecompileError::MIP403RegistryError);
    add_errors_to_registry(&mut registry, MagnusPrecompileError::FeeManagerError);
    add_errors_to_registry(&mut registry, MagnusPrecompileError::MIPFeeAMMError);
    add_errors_to_registry(&mut registry, MagnusPrecompileError::NonceError);
    add_errors_to_registry(&mut registry, MagnusPrecompileError::ValidatorConfigError);
    add_errors_to_registry(&mut registry, MagnusPrecompileError::AccountKeychainError);

    registry
}

pub static ERROR_REGISTRY: LazyLock<MagnusPrecompileErrorRegistry> =
    LazyLock::new(error_decoder_registry);

/// Decode an error from raw bytes using the selector
pub fn decode_error<'a>(data: &'a [u8]) -> Option<DecodedMagnusPrecompileError<'a>> {
    if data.len() < 4 {
        return None;
    }

    let selector: [u8; 4] = data[0..4].try_into().ok()?;
    ERROR_REGISTRY
        .get(&selector)
        .and_then(|decoder| decoder(data))
}

/// Extension trait to convert `Result<T, MagnusPrecompileError` into `PrecompileResult`
pub trait IntoPrecompileResult<T> {
    fn into_precompile_result(
        self,
        gas: u64,
        encode_ok: impl FnOnce(T) -> alloy::primitives::Bytes,
    ) -> PrecompileResult;
}

impl<T> IntoPrecompileResult<T> for Result<T> {
    fn into_precompile_result(
        self,
        gas: u64,
        encode_ok: impl FnOnce(T) -> alloy::primitives::Bytes,
    ) -> PrecompileResult {
        match self {
            Ok(res) => Ok(PrecompileOutput::new(gas, encode_ok(res))),
            Err(err) => err.into_precompile_result(gas),
        }
    }
}

impl<T> IntoPrecompileResult<T> for MagnusPrecompileError {
    fn into_precompile_result(
        self,
        gas: u64,
        _encode_ok: impl FnOnce(T) -> alloy::primitives::Bytes,
    ) -> PrecompileResult {
        Self::into_precompile_result(self, gas)
    }
}
