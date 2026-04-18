//! Magnus Node types config.

#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub use magnus_payload_types::{MagnusExecutionData, MagnusPayloadTypes};
pub use version::{init_version_metadata, version_metadata};

use crate::node::{MagnusAddOns, MagnusNode};
pub use crate::node::{MagnusNodeArgs, MagnusPoolBuilder};
use reth_ethereum::provider::db::DatabaseEnv;
use reth_node_builder::{FullNode, NodeAdapter, RethFullAdapter};
pub use magnus_transaction_pool::validator::DEFAULT_AA_VALID_AFTER_MAX_SECS;

pub mod engine;
pub mod node;
pub mod rpc;
pub mod telemetry;
pub use magnus_consensus as consensus;
pub use magnus_evm as evm;
pub use magnus_primitives as primitives;

mod version;

type MagnusFullNodeTypes = RethFullAdapter<DatabaseEnv, MagnusNode>;
type MagnusNodeAdapter = NodeAdapter<MagnusFullNodeTypes>;

/// Type alias for a launched magnus node.
pub type MagnusFullNode = FullNode<MagnusNodeAdapter, MagnusAddOns<MagnusFullNodeTypes>>;
