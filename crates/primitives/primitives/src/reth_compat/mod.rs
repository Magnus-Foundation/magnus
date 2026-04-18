//! Reth-specific trait implementations for Magnus primitives.
//!
//! This module consolidates all `reth`/`reth-codec`/`serde-bincode-compat` trait
//! implementations so they can be cleanly removed when publishing crates
//! without reth dependencies.

use alloy_primitives::Log;
use reth_ethereum_primitives::EthereumReceipt;
use reth_primitives_traits::NodePrimitives;

use crate::{Block, BlockBody, MagnusHeader, MagnusPrimitives, MagnusTxEnvelope, MagnusTxType};

/// Magnus receipt.
///
/// Re-export from `reth_ethereum_primitives` so that the rest of the workspace crates see a single
/// type that satisfies both alloy trait bounds and reth trait bounds.
///
/// Shadows the alloy-only alias in `lib.rs` when the `reth` feature is active.
pub type MagnusReceipt<L = Log> = EthereumReceipt<MagnusTxType, L>;

impl NodePrimitives for MagnusPrimitives {
    type Block = Block;
    type BlockHeader = MagnusHeader;
    type BlockBody = BlockBody;
    type SignedTx = MagnusTxEnvelope;
    type Receipt = MagnusReceipt;
}

mod ed25519;

mod header;

mod subblock;

pub(crate) mod transaction;
