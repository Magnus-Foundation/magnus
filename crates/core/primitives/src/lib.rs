//! Magnus primitive types

#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg), allow(unexpected_cfgs))]

pub use alloy_consensus::Header;

pub mod transaction;
pub use transaction::{
    AASigned, MAX_WEBAUTHN_SIGNATURE_LENGTH, P256_SIGNATURE_LENGTH, SECP256K1_SIGNATURE_LENGTH,
    SignatureType, MAGNUS_GAS_PRICE_SCALING_FACTOR, MAGNUS_TX_TYPE_ID, MagnusSignature,
    MagnusTransaction, MagnusTxEnvelope, MagnusTxType, derive_p256_address,
};

mod header;
pub use header::MagnusHeader;

pub mod subblock;
pub use subblock::{
    RecoveredSubBlock, SignedSubBlock, SubBlock, SubBlockMetadata, SubBlockVersion,
};

#[cfg(feature = "reth")]
use alloy_primitives::Log;
#[cfg(feature = "reth")]
use reth_ethereum_primitives::EthereumReceipt;
#[cfg(feature = "reth")]
use reth_primitives_traits::NodePrimitives;

/// Magnus block.
pub type Block = alloy_consensus::Block<MagnusTxEnvelope, MagnusHeader>;

/// Magnus block body.
pub type BlockBody = alloy_consensus::BlockBody<MagnusTxEnvelope, MagnusHeader>;

/// Magnus receipt.
#[cfg(feature = "reth")]
pub type MagnusReceipt<L = Log> = EthereumReceipt<MagnusTxType, L>;

/// A [`NodePrimitives`] implementation for Magnus.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
#[non_exhaustive]
pub struct MagnusPrimitives;

#[cfg(feature = "reth")]
impl NodePrimitives for MagnusPrimitives {
    type Block = Block;
    type BlockHeader = MagnusHeader;
    type BlockBody = BlockBody;
    type SignedTx = MagnusTxEnvelope;
    type Receipt = MagnusReceipt;
}
