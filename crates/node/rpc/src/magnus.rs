//! Magnus-specific JSON-RPC API implementation.

use std::sync::Arc;

use jsonrpsee::{core::RpcResult, proc_macros::rpc};

use crate::state::{NodeState, NodeStatus};

/// Magnus-specific JSON-RPC API trait.
///
/// Provides methods specific to Magnus node operations.
#[rpc(server, namespace = "magnus")]
pub trait MagnusApi {
    /// Returns the current node status including consensus information.
    #[method(name = "nodeStatus")]
    async fn node_status(&self) -> RpcResult<NodeStatus>;
}

/// Implementation of the Magnus RPC API.
#[derive(Debug)]
pub struct MagnusApiImpl {
    state: Arc<NodeState>,
}

impl MagnusApiImpl {
    /// Create a new Magnus API implementation.
    #[must_use]
    pub const fn new(state: Arc<NodeState>) -> Self {
        Self { state }
    }
}

#[jsonrpsee::core::async_trait]
impl MagnusApiServer for MagnusApiImpl {
    async fn node_status(&self) -> RpcResult<NodeStatus> {
        Ok(self.state.status())
    }
}
