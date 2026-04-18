//! Magnus primitive types

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg), allow(unexpected_cfgs))]

pub use alloy_consensus::Header;

mod address;
pub use address::{MasterId, MagnusAddressExt, UserTag, is_tip20_prefix};
pub mod ed25519;

pub mod transaction;
pub use transaction::{
    AASigned, MAX_WEBAUTHN_SIGNATURE_LENGTH, P256_SIGNATURE_LENGTH, SECP256K1_SIGNATURE_LENGTH,
    SignatureType, MAGNUS_GAS_PRICE_SCALING_FACTOR, MAGNUS_TX_TYPE_ID, MagnusSignature,
    MagnusTransaction, MagnusTxEnvelope, MagnusTxType, derive_p256_address,
};

mod header;
pub use header::{MagnusConsensusContext, MagnusHeader};

pub mod subblock;
pub use subblock::{
    RecoveredSubBlock, SignedSubBlock, SubBlock, SubBlockMetadata, SubBlockVersion,
};

extern crate alloc;

use once_cell as _;

/// Magnus block.
pub type Block = alloy_consensus::Block<MagnusTxEnvelope, MagnusHeader>;

/// Magnus block body.
pub type BlockBody = alloy_consensus::BlockBody<MagnusTxEnvelope, MagnusHeader>;

#[cfg(feature = "reth")]
mod reth_compat;

/// Magnus receipt.
/// Implements reth trait bounds when the `reth` feature is enabled.
#[cfg(feature = "reth")]
pub use reth_compat::MagnusReceipt;
#[cfg(not(feature = "reth"))]
pub type MagnusReceipt<L = alloy_primitives::Log> = alloy_consensus::EthereumReceipt<MagnusTxType, L>;

/// Marker type for Magnus node primitives.
/// Implements [`reth_primitives_traits::NodePrimitives`] when the `reth` feature is enabled.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
#[non_exhaustive]
pub struct MagnusPrimitives;
