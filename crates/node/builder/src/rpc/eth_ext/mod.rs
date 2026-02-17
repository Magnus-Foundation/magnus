use crate::rpc::eth_ext::transactions::TransactionsResponse;
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use magnus_indexer::BlockIndex;
use reth_node_core::rpc::result::internal_rpc_err;
use reth_rpc_eth_api::RpcNodeCore;
use magnus_provider::rpc::pagination::PaginationParams;
use std::sync::Arc;

pub mod transactions;
pub use transactions::TransactionsFilter;

#[rpc(server, namespace = "eth")]
pub trait MagnusEthExtApi {
    /// Gets paginated transactions on Magnus with flexible filtering and sorting.
    ///
    /// Uses cursor-based pagination for stable iteration through transactions.
    #[method(name = "getTransactions")]
    async fn transactions(
        &self,
        params: PaginationParams<TransactionsFilter>,
    ) -> RpcResult<TransactionsResponse>;
}

/// The JSON-RPC handlers for the `eth_` ext namespace.
#[derive(Debug, Clone)]
pub struct MagnusEthExt<EthApi> {
    eth_api: EthApi,
    block_index: Arc<BlockIndex>,
}

impl<EthApi> MagnusEthExt<EthApi> {
    /// Creates a new `MagnusEthExt` handler backed by the given block index.
    pub fn new(eth_api: EthApi, block_index: Arc<BlockIndex>) -> Self {
        Self { eth_api, block_index }
    }
}

#[async_trait::async_trait]
impl<EthApi: RpcNodeCore> MagnusEthExtApiServer for MagnusEthExt<EthApi> {
    async fn transactions(
        &self,
        _params: PaginationParams<TransactionsFilter>,
    ) -> RpcResult<TransactionsResponse> {
        Err(internal_rpc_err("unimplemented"))
    }
}

impl<EthApi: RpcNodeCore> MagnusEthExt<EthApi> {
    /// Access the underlying provider.
    pub fn provider(&self) -> &EthApi::Provider {
        self.eth_api.provider()
    }
}
