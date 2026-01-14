//! Magnus Node types config.

#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub use magnus_payload_types::{MagnusExecutionData, MagnusPayloadTypes};
pub use version::{init_version_metadata, version_metadata};

pub use crate::node::{DEFAULT_AA_VALID_AFTER_MAX_SECS, MagnusNodeArgs, MagnusPoolBuilder};
use crate::node::{MagnusAddOns, MagnusNode};
use reth_ethereum::provider::db::DatabaseEnv;
use reth_node_builder::{FullNode, NodeAdapter, RethFullAdapter};
use std::sync::Arc;

pub mod engine;
pub mod node;
pub mod rpc;
pub use magnus_consensus as consensus;
pub use magnus_evm as evm;
pub use magnus_primitives as primitives;

mod version;

type MagnusNodeAdapter = NodeAdapter<RethFullAdapter<Arc<DatabaseEnv>, MagnusNode>>;

/// Type alias for a launched Magnus node.
pub type MagnusFullNode = FullNode<MagnusNodeAdapter, MagnusAddOns<MagnusNodeAdapter>>;
