//! Unified error handling for Magnus precompiles.
//!
//! Provides [`MagnusPrecompileError`] — the top-level error enum — along with an
//! ABI-selector-based decoder registry for mapping raw revert bytes back to
//! typed error variants.

use std::{
    collections::HashMap,
    sync::{Arc, LazyLock},
};

use crate::mip20::MIP20Error;
use alloy::{
    primitives::{Selector, U256},
    sol_types::{Panic, PanicKind, SolError, SolInterface},
};
use alloy_evm::EvmInternalsError;
use revm::{
    context::journaled_state::JournalLoadError,
    precompile::{PrecompileError, PrecompileHalt, PrecompileOutput, PrecompileResult},
};
use magnus_contracts::precompiles::{
    AccountKeychainError, AddrRegistryError, FeeManagerError, NonceError, RolesAuthError,
    SignatureVerifierError, StablecoinDEXError, MIP20FactoryError, MIP20IssuerRegistryError,
    MIP403RegistryError, TIPFeeAMMError, UnknownFunctionSelector, ValidatorConfigError,
    ValidatorConfigV2Error,
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

    /// Error from MIP20 issuer registry (multi-currency-fees-design.md §4)
    #[error("MIP20 issuer registry error: {0:?}")]
    MIP20IssuerRegistry(MIP20IssuerRegistryError),

    /// Error from roles auth
    #[error("Roles auth error: {0:?}")]
    RolesAuthError(RolesAuthError),

    /// Error from MIP20 registry (MIP-1022)
    #[error("MIP20 registry error: {0:?}")]
    AddrRegistryError(AddrRegistryError),

    /// Error from 403 registry
    #[error("MIP403 registry error: {0:?}")]
    MIP403RegistryError(MIP403RegistryError),

    /// Error from MIP fee manager
    #[error("MIP fee manager error: {0:?}")]
    FeeManagerError(FeeManagerError),

    /// Error from MIP fee AMM
    #[error("MIP fee AMM error: {0:?}")]
    TIPFeeAMMError(TIPFeeAMMError),

    /// Error from Magnus Transaction nonce manager
    #[error("Magnus Transaction nonce error: {0:?}")]
    NonceError(NonceError),

    /// EVM panic (i.e. arithmetic under/overflow, out-of-bounds access).
    #[error("Panic({0:?})")]
    Panic(PanicKind),

    /// Error from validator config
    #[error("Validator config error: {0:?}")]
    ValidatorConfigError(ValidatorConfigError),

    /// Error from validator config v2
    #[error("Validator config v2 error: {0:?}")]
    ValidatorConfigV2Error(ValidatorConfigV2Error),

    /// Error from account keychain precompile
    #[error("Account keychain error: {0:?}")]
    AccountKeychainError(AccountKeychainError),

    /// Error from signature verifier precompile
    #[error("Signature verifier error: {0:?}")]
    SignatureVerifierError(SignatureVerifierError),

    /// Gas limit exceeded during precompile execution.
    #[error("Gas limit exceeded")]
    OutOfGas,

    /// The calldata's 4-byte selector does not match any known precompile function.
    #[error("Unknown function selector: {0:?}")]
    UnknownFunctionSelector([u8; 4]),

    /// Unrecoverable internal error (e.g. database failure).
    #[error("Fatal precompile error: {0:?}")]
    #[from(skip)]
    Fatal(String),
}

impl From<EvmInternalsError> for MagnusPrecompileError {
    fn from(value: EvmInternalsError) -> Self {
        match value {
            EvmInternalsError::Database(e) => Self::Fatal(e.to_string()),
        }
    }
}

impl From<JournalLoadError<EvmInternalsError>> for MagnusPrecompileError {
    fn from(value: JournalLoadError<EvmInternalsError>) -> Self {
        match value {
            JournalLoadError::DBError(e) => Self::from(e),
            JournalLoadError::ColdLoadSkipped => Self::OutOfGas,
        }
    }
}

impl From<JournalLoadError<revm::context::ErasedError>> for MagnusPrecompileError {
    fn from(value: JournalLoadError<revm::context::ErasedError>) -> Self {
        match value {
            JournalLoadError::DBError(e) => Self::Fatal(e.to_string()),
            JournalLoadError::ColdLoadSkipped => Self::OutOfGas,
        }
    }
}

/// Result type alias for Magnus precompile operations
pub type Result<T> = std::result::Result<T, MagnusPrecompileError>;

impl MagnusPrecompileError {
    /// Returns true if this error represents a system-level failure that must be propagated
    /// rather than swallowed, because state may be inconsistent.
    pub fn is_system_error(&self) -> bool {
        match self {
            Self::OutOfGas | Self::Fatal(_) | Self::Panic(_) => true,
            Self::StablecoinDEX(_)
            | Self::MIP20(_)
            | Self::NonceError(_)
            | Self::MIP20Factory(_)
            | Self::MIP20IssuerRegistry(_)
            | Self::RolesAuthError(_)
            | Self::AddrRegistryError(_)
            | Self::TIPFeeAMMError(_)
            | Self::FeeManagerError(_)
            | Self::MIP403RegistryError(_)
            | Self::ValidatorConfigError(_)
            | Self::ValidatorConfigV2Error(_)
            | Self::AccountKeychainError(_)
            | Self::SignatureVerifierError(_)
            | Self::UnknownFunctionSelector(_) => false,
        }
    }

    /// Creates an arithmetic under/overflow panic error.
    pub fn under_overflow() -> Self {
        Self::Panic(PanicKind::UnderOverflow)
    }

    /// Creates an enum conversion error panic (Solidity Panic `0x21`).
    pub fn enum_conversion_error() -> Self {
        Self::Panic(PanicKind::EnumConversionError)
    }

    /// Creates an array out-of-bounds panic error.
    pub fn array_oob() -> Self {
        Self::Panic(PanicKind::ArrayOutOfBounds)
    }

    /// ABI-encodes this error and wraps it as a reverted [`PrecompileResult`].
    ///
    /// # Errors
    /// - `PrecompileOutput::halt(PrecompileHalt::OutOfGas, ..)` — if the variant is [`OutOfGas`](Self::OutOfGas)
    /// - `PrecompileError::Fatal` — if the variant is [`Fatal`](Self::Fatal)
    pub fn into_precompile_result(self, gas: u64, reservoir: u64) -> PrecompileResult {
        let bytes = match self {
            Self::StablecoinDEX(e) => e.abi_encode().into(),
            Self::MIP20(e) => e.abi_encode().into(),
            Self::MIP20Factory(e) => e.abi_encode().into(),
            Self::MIP20IssuerRegistry(e) => e.abi_encode().into(),
            Self::RolesAuthError(e) => e.abi_encode().into(),
            Self::AddrRegistryError(e) => e.abi_encode().into(),
            Self::MIP403RegistryError(e) => e.abi_encode().into(),
            Self::FeeManagerError(e) => e.abi_encode().into(),
            Self::TIPFeeAMMError(e) => e.abi_encode().into(),
            Self::NonceError(e) => e.abi_encode().into(),
            Self::Panic(kind) => {
                let panic = Panic {
                    code: U256::from(kind as u32),
                };

                panic.abi_encode().into()
            }
            Self::ValidatorConfigError(e) => e.abi_encode().into(),
            Self::ValidatorConfigV2Error(e) => e.abi_encode().into(),
            Self::AccountKeychainError(e) => e.abi_encode().into(),
            Self::SignatureVerifierError(e) => e.abi_encode().into(),
            Self::OutOfGas => {
                return Ok(PrecompileOutput::halt(PrecompileHalt::OutOfGas, reservoir));
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
        Ok(PrecompileOutput::revert(gas, bytes, reservoir))
    }
}

/// Registers all ABI error selectors for a [`SolInterface`] type into the decoder registry.
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

/// A decoded precompile error together with the raw revert bytes.
pub struct DecodedMagnusPrecompileError<'a> {
    pub error: MagnusPrecompileError,
    pub revert_bytes: &'a [u8],
}

/// Maps ABI error selectors to their decoder functions.
pub type MagnusPrecompileErrorRegistry = HashMap<
    Selector,
    Box<dyn for<'a> Fn(&'a [u8]) -> Option<DecodedMagnusPrecompileError<'a>> + Send + Sync>,
>;

/// Builds a [`MagnusPrecompileErrorRegistry`] mapping every known error selector to its decoder.
pub fn error_decoder_registry() -> MagnusPrecompileErrorRegistry {
    let mut registry: MagnusPrecompileErrorRegistry = HashMap::new();

    add_errors_to_registry(&mut registry, MagnusPrecompileError::StablecoinDEX);
    add_errors_to_registry(&mut registry, MagnusPrecompileError::MIP20);
    add_errors_to_registry(&mut registry, MagnusPrecompileError::MIP20Factory);
    add_errors_to_registry(&mut registry, MagnusPrecompileError::MIP20IssuerRegistry);
    add_errors_to_registry(&mut registry, MagnusPrecompileError::RolesAuthError);
    add_errors_to_registry(&mut registry, MagnusPrecompileError::AddrRegistryError);
    add_errors_to_registry(&mut registry, MagnusPrecompileError::MIP403RegistryError);
    add_errors_to_registry(&mut registry, MagnusPrecompileError::FeeManagerError);
    add_errors_to_registry(&mut registry, MagnusPrecompileError::TIPFeeAMMError);
    add_errors_to_registry(&mut registry, MagnusPrecompileError::NonceError);
    add_errors_to_registry(&mut registry, MagnusPrecompileError::ValidatorConfigError);
    add_errors_to_registry(&mut registry, MagnusPrecompileError::ValidatorConfigV2Error);
    add_errors_to_registry(&mut registry, MagnusPrecompileError::AccountKeychainError);
    add_errors_to_registry(&mut registry, MagnusPrecompileError::SignatureVerifierError);

    registry
}

/// Global lazily-initialized registry of all Magnus precompile error decoders.
pub static ERROR_REGISTRY: LazyLock<MagnusPrecompileErrorRegistry> =
    LazyLock::new(error_decoder_registry);

/// Decodes raw revert bytes into a typed [`DecodedMagnusPrecompileError`] using the global
/// [`ERROR_REGISTRY`], returning `None` if the data is shorter than 4 bytes or the selector
/// is unrecognized.
pub fn decode_error<'a>(data: &'a [u8]) -> Option<DecodedMagnusPrecompileError<'a>> {
    if data.len() < 4 {
        return None;
    }

    let selector: [u8; 4] = data[0..4].try_into().ok()?;
    ERROR_REGISTRY
        .get(&selector)
        .and_then(|decoder| decoder(data))
}

/// Extension trait to convert `Result<T, MagnusPrecompileError>` into a [`PrecompileResult`].
pub trait IntoPrecompileResult<T> {
    /// Converts `self` into a [`PrecompileResult`], using `encode_ok` for the success path.
    fn into_precompile_result(
        self,
        gas: u64,
        reservoir: u64,
        encode_ok: impl FnOnce(T) -> alloy::primitives::Bytes,
    ) -> PrecompileResult;
}

impl<T> IntoPrecompileResult<T> for Result<T> {
    fn into_precompile_result(
        self,
        gas: u64,
        reservoir: u64,
        encode_ok: impl FnOnce(T) -> alloy::primitives::Bytes,
    ) -> PrecompileResult {
        match self {
            Ok(res) => Ok(PrecompileOutput::new(gas, encode_ok(res), reservoir)),
            Err(err) => err.into_precompile_result(gas, reservoir),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use magnus_contracts::precompiles::StablecoinDEXError;

    #[test]
    fn test_add_errors_to_registry_populates_registry() {
        let mut registry: MagnusPrecompileErrorRegistry = HashMap::new();

        assert!(registry.is_empty());

        add_errors_to_registry(&mut registry, MagnusPrecompileError::StablecoinDEX);

        assert!(!registry.is_empty());

        let order_not_found_selector = StablecoinDEXError::order_does_not_exist().selector();
        assert!(
            registry.contains_key(&order_not_found_selector),
            "Registry should contain OrderDoesNotExist selector"
        );
    }

    #[test]
    fn test_error_decoder_registry_is_not_empty() {
        let registry = error_decoder_registry();

        assert!(
            !registry.is_empty(),
            "error_decoder_registry should return a populated registry"
        );

        let dex_selector = StablecoinDEXError::order_does_not_exist().selector();
        assert!(registry.contains_key(&dex_selector));
    }

    #[test]
    fn test_decode_error_returns_some_for_valid_error() {
        let error = StablecoinDEXError::order_does_not_exist();
        let encoded = error.abi_encode();

        let result = decode_error(&encoded);
        assert!(
            result.is_some(),
            "decode_error should return Some for valid error"
        );

        let decoded = result.unwrap();
        assert!(matches!(
            decoded.error,
            MagnusPrecompileError::StablecoinDEX(StablecoinDEXError::OrderDoesNotExist(_))
        ));
    }

    /// Verifies the new MIP20IssuerRegistry errors round-trip through the
    /// global decoder registry. If the registry is missing the new variant,
    /// decode_error would return None for valid registry-error revert bytes.
    #[test]
    fn test_decode_error_handles_issuer_registry_errors() {
        use alloy_primitives::Address;
        use magnus_contracts::precompiles::MIP20IssuerRegistryError;

        let issuer = Address::repeat_byte(0xAA);
        let err = MIP20IssuerRegistryError::issuer_not_approved(issuer, "USD".into());
        let encoded = err.abi_encode();

        let decoded = decode_error(&encoded).expect("decoder must recognize IssuerNotApproved");
        match decoded.error {
            MagnusPrecompileError::MIP20IssuerRegistry(
                MIP20IssuerRegistryError::IssuerNotApproved(inner),
            ) => {
                assert_eq!(inner.issuer, issuer);
                assert_eq!(inner.currency, "USD");
            }
            other => panic!(
                "expected MIP20IssuerRegistry::IssuerNotApproved, got {other:?}"
            ),
        }
    }

    /// Verifies the new FeeManager errors (G0 additions) round-trip through
    /// the global decoder registry.
    #[test]
    fn test_decode_error_handles_new_fee_manager_errors() {
        use alloy_primitives::Address;
        use magnus_contracts::precompiles::FeeManagerError;

        // CurrencyNotRegistered
        let err = FeeManagerError::currency_not_registered("XYZ".into());
        let encoded = err.abi_encode();
        let decoded =
            decode_error(&encoded).expect("decoder must recognize CurrencyNotRegistered");
        assert!(matches!(
            decoded.error,
            MagnusPrecompileError::FeeManagerError(FeeManagerError::CurrencyNotRegistered(_))
        ));

        // FeeTokenNotInferable (unit error)
        let err = FeeManagerError::fee_token_not_inferable();
        let encoded = err.abi_encode();
        let decoded =
            decode_error(&encoded).expect("decoder must recognize FeeTokenNotInferable");
        assert!(matches!(
            decoded.error,
            MagnusPrecompileError::FeeManagerError(FeeManagerError::FeeTokenNotInferable(_))
        ));

        // FeeTokenNotAccepted (with field check)
        let validator = Address::repeat_byte(0xBB);
        let token = Address::repeat_byte(0xCC);
        let err = FeeManagerError::fee_token_not_accepted(validator, token);
        let encoded = err.abi_encode();
        let decoded =
            decode_error(&encoded).expect("decoder must recognize FeeTokenNotAccepted");
        match decoded.error {
            MagnusPrecompileError::FeeManagerError(
                FeeManagerError::FeeTokenNotAccepted(inner),
            ) => {
                assert_eq!(inner.validator, validator);
                assert_eq!(inner.token, token);
            }
            other => panic!("expected FeeTokenNotAccepted, got {other:?}"),
        }
    }

    // Note: `IssuerNotApproved(address,string)` is defined only on the
    // IssuerRegistry ABI (not on the factory). The factory bubbles up the
    // registry's error directly when its issuer-allowlist gate fails, which
    // avoids a Solidity selector collision (two errors with identical
    // signatures would produce the same 4-byte selector and ambiguate the
    // decoder). The handler-roundtrip test for that error lives in
    // `test_decode_error_handles_issuer_registry_errors` above.

    #[test]
    fn test_decode_error_data_length_boundary() {
        // Empty data (len = 0) should return None (0 < 4)
        let result = decode_error(&[]);
        assert!(result.is_none(), "Empty data should return None");

        // 1 byte (len = 1) should return None (1 < 4)
        let result = decode_error(&[0x01]);
        assert!(result.is_none(), "1 byte should return None");

        // 2 bytes (len = 2) should return None (2 < 4)
        let result = decode_error(&[0x01, 0x02]);
        assert!(result.is_none(), "2 bytes should return None");

        // 3 bytes (len = 3) should return None (3 < 4)
        let result = decode_error(&[0x01, 0x02, 0x03]);
        assert!(result.is_none(), "3 bytes should return None");

        // 4 bytes with unknown selector returns None (selector not found)
        let result = decode_error(&[0x00, 0x00, 0x00, 0x00]);
        assert!(
            result.is_none(),
            "Unknown 4-byte selector should return None"
        );

        // 4 bytes with valid selector (exactly at boundary) should succeed
        let error = StablecoinDEXError::order_does_not_exist();
        let encoded = error.abi_encode();
        let result = decode_error(&encoded);
        assert!(
            result.is_some(),
            "Valid error at 4+ bytes should return Some"
        );
    }

    #[test]
    fn test_decode_error_with_tip20_error() {
        // Use insufficient_allowance which has a unique selector (no collision with other errors)
        let error = MIP20Error::insufficient_allowance();
        let encoded = error.abi_encode();

        let result = decode_error(&encoded);
        assert!(result.is_some(), "Should decode MIP20 errors");

        let decoded = result.unwrap();
        // Verify it's a MIP20 error
        match decoded.error {
            MagnusPrecompileError::MIP20(_) => {}
            other => panic!("Expected MIP20 error, got {other:?}"),
        }
    }
}
