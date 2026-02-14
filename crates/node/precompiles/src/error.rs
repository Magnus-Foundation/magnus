//! Precompile error types.

use revm::precompile::{PrecompileOutput, PrecompileResult};

/// Precompile result alias.
pub type Result<T> = core::result::Result<T, MagnusPrecompileError>;

/// Precompile errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MagnusPrecompileError {
    /// Unknown function selector.
    #[error("unknown function selector: {0:?}")]
    UnknownSelector([u8; 4]),

    /// Insufficient balance.
    #[error("insufficient balance")]
    InsufficientBalance,

    /// Overflow or underflow.
    #[error("overflow or underflow")]
    Overflow,

    /// Unauthorized operation.
    #[error("unauthorized: {0}")]
    Unauthorized(String),

    /// Invalid input.
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// Oracle error.
    #[error("oracle: {0}")]
    Oracle(String),

    /// Fee error.
    #[error("fee: {0}")]
    Fee(String),
}

/// Convert [`Result<T>`] into [`PrecompileResult`] with ABI-encoded return data.
pub trait IntoPrecompileResult<T> {
    /// Converts the result into a [`PrecompileResult`], encoding the success value
    /// or producing a reverted output on error.
    fn into_precompile_result(
        self,
        gas_used: u64,
        encode: impl FnOnce(T) -> alloy_primitives::Bytes,
    ) -> PrecompileResult;
}

impl<T> IntoPrecompileResult<T> for Result<T> {
    fn into_precompile_result(
        self,
        gas_used: u64,
        encode: impl FnOnce(T) -> alloy_primitives::Bytes,
    ) -> PrecompileResult {
        match self {
            Ok(val) => Ok(PrecompileOutput::new(gas_used, encode(val))),
            Err(e) => Ok(PrecompileOutput::new_reverted(
                gas_used,
                alloy_primitives::Bytes::from(format!("{e}").into_bytes()),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display() {
        let err = MagnusPrecompileError::InsufficientBalance;
        assert_eq!(err.to_string(), "insufficient balance");
    }

    #[test]
    fn result_ok_encodes() {
        let result: Result<u64> = Ok(42);
        let precompile_result =
            result.into_precompile_result(100, |v| alloy_primitives::Bytes::from(format!("{v}").into_bytes()));
        let output = precompile_result.unwrap();
        assert!(!output.reverted);
        assert_eq!(output.gas_used, 100);
    }

    #[test]
    fn result_err_reverts() {
        let result: Result<u64> = Err(MagnusPrecompileError::InsufficientBalance);
        let precompile_result =
            result.into_precompile_result(50, |v| alloy_primitives::Bytes::from(format!("{v}").into_bytes()));
        let output = precompile_result.unwrap();
        assert!(output.reverted);
    }
}
