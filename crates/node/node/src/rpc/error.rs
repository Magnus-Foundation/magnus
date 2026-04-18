use std::convert::Infallible;

use alloy_primitives::Bytes;
use reth_errors::ProviderError;
use reth_evm::revm::context::result::EVMError;
use reth_node_core::rpc::result::rpc_err;
use reth_rpc_eth_api::AsEthApiError;
use reth_rpc_eth_types::{
    EthApiError,
    error::api::{FromEvmHalt, FromRevert},
};
use magnus_evm::MagnusHaltReason;

#[derive(Debug, thiserror::Error)]
pub enum MagnusEthApiError {
    #[error(transparent)]
    EthApiError(EthApiError),
}

impl From<MagnusEthApiError> for jsonrpsee::types::error::ErrorObject<'static> {
    fn from(error: MagnusEthApiError) -> Self {
        match error {
            MagnusEthApiError::EthApiError(err) => err.into(),
        }
    }
}
impl From<Infallible> for MagnusEthApiError {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

impl AsEthApiError for MagnusEthApiError {
    fn as_err(&self) -> Option<&EthApiError> {
        match self {
            Self::EthApiError(err) => Some(err),
        }
    }
}

impl From<EthApiError> for MagnusEthApiError {
    fn from(error: EthApiError) -> Self {
        Self::EthApiError(error)
    }
}

impl From<ProviderError> for MagnusEthApiError {
    fn from(error: ProviderError) -> Self {
        EthApiError::from(error).into()
    }
}
impl<T, TxError> From<EVMError<T, TxError>> for MagnusEthApiError
where
    T: Into<EthApiError>,
    TxError: reth_evm::InvalidTxError,
{
    fn from(error: EVMError<T, TxError>) -> Self {
        EthApiError::from(error).into()
    }
}

impl FromEvmHalt<MagnusHaltReason> for MagnusEthApiError {
    fn from_evm_halt(halt: MagnusHaltReason, gas_limit: u64) -> Self {
        EthApiError::from_evm_halt(halt, gas_limit).into()
    }
}

impl FromRevert for MagnusEthApiError {
    fn from_revert(revert: Bytes) -> Self {
        match magnus_precompiles::error::decode_error(&revert.0) {
            Some(error) => Self::EthApiError(EthApiError::Other(Box::new(rpc_err(
                3,
                format!("execution reverted: {}", error.error),
                Some(error.revert_bytes),
            )))),
            None => Self::EthApiError(EthApiError::from_revert(revert)),
        }
    }
}
