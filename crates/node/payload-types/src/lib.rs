//! Magnus payload types.

#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod attrs;

use alloy_primitives::B256;
pub use attrs::{InterruptHandle, MagnusPayloadAttributes, MagnusPayloadBuilderAttributes};
use std::sync::Arc;

use alloy_rpc_types_eth::Withdrawal;
use reth_ethereum_engine_primitives::EthBuiltPayload;
use reth_node_api::{ExecutionPayload, PayloadBuilderAttributes, PayloadTypes};
use reth_primitives_traits::{AlloyBlockHeader as _, SealedBlock};
use serde::{Deserialize, Serialize};
use magnus_primitives::{Block, MagnusPrimitives};

/// Payload types for Magnus node.
#[derive(Debug, Clone, Copy, Default)]
#[non_exhaustive]
pub struct MagnusPayloadTypes;

/// Execution data for Magnus node. Simply wraps a sealed block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MagnusExecutionData {
    /// The built block.
    pub block: Arc<SealedBlock<Block>>,
    /// Validator set active at the time this block was built.
    pub validator_set: Option<Vec<B256>>,
}

impl ExecutionPayload for MagnusExecutionData {
    fn parent_hash(&self) -> alloy_primitives::B256 {
        self.block.parent_hash()
    }

    fn block_hash(&self) -> alloy_primitives::B256 {
        self.block.hash()
    }

    fn block_number(&self) -> u64 {
        self.block.number()
    }

    fn withdrawals(&self) -> Option<&Vec<Withdrawal>> {
        self.block
            .body()
            .withdrawals
            .as_ref()
            .map(|withdrawals| &withdrawals.0)
    }

    fn parent_beacon_block_root(&self) -> Option<alloy_primitives::B256> {
        self.block.parent_beacon_block_root()
    }

    fn timestamp(&self) -> u64 {
        self.block.timestamp()
    }

    fn gas_used(&self) -> u64 {
        self.block.gas_used()
    }

    fn block_access_list(&self) -> Option<&alloy_primitives::Bytes> {
        None
    }

    fn transaction_count(&self) -> usize {
        self.block.body().transactions.len()
    }
}

impl PayloadTypes for MagnusPayloadTypes {
    type ExecutionData = MagnusExecutionData;
    type BuiltPayload = EthBuiltPayload<MagnusPrimitives>;
    type PayloadAttributes =
        <Self::PayloadBuilderAttributes as PayloadBuilderAttributes>::RpcPayloadAttributes;
    type PayloadBuilderAttributes = MagnusPayloadBuilderAttributes;

    fn block_to_payload(block: SealedBlock<Block>) -> Self::ExecutionData {
        MagnusExecutionData {
            block: Arc::new(block),
            validator_set: None,
        }
    }
}
