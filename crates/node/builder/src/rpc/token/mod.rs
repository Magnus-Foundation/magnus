use crate::rpc::token::{
    role_history::{RoleHistoryFilters, RoleHistoryResponse},
    tokens::{TokensFilters, TokensResponse},
    tokens_by_address::{TokensByAddressParams, TokensByAddressResponse},
};
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use magnus_indexer::TokenStore;
use reth_node_core::rpc::result::internal_rpc_err;
use reth_rpc_eth_api::RpcNodeCore;
use magnus_provider::rpc::pagination::PaginationParams;
use std::sync::Arc;

pub mod role_history;
pub mod tokens;
pub mod tokens_by_address;

#[rpc(server, namespace = "token")]
pub trait MagnusTokenApi {
    /// Gets paginated role change history for MIP-20 tokens on Magnus.
    ///
    /// Tracks role grants and revocations from the RoleMembershipUpdated event for audit trails and compliance monitoring.
    ///
    /// Uses cursor-based pagination for stable iteration through role changes.
    #[method(name = "getRoleHistory")]
    async fn role_history(
        &self,
        params: PaginationParams<RoleHistoryFilters>,
    ) -> RpcResult<RoleHistoryResponse>;

    /// Gets paginated MIP-20 tokens on Magnus.
    ///
    /// Uses cursor-based pagination for stable iteration through tokens.
    #[method(name = "getTokens")]
    async fn tokens(&self, params: PaginationParams<TokensFilters>) -> RpcResult<TokensResponse>;

    /// Gets paginated MIP-20 tokens associated with an account address on Magnus.
    ///
    /// Returns tokens where the account has a balance or specific roles.
    ///
    /// Uses cursor-based pagination for stable iteration through tokens.
    #[method(name = "getTokensByAddress")]
    async fn tokens_by_address(
        &self,
        params: TokensByAddressParams,
    ) -> RpcResult<TokensByAddressResponse>;
}

/// The JSON-RPC handlers for the `token_` namespace.
#[derive(Debug, Clone)]
pub struct MagnusToken<EthApi> {
    eth_api: EthApi,
    token_store: Arc<TokenStore>,
}

impl<EthApi> MagnusToken<EthApi> {
    /// Creates a new `MagnusToken` handler backed by the given token store.
    pub fn new(eth_api: EthApi, token_store: Arc<TokenStore>) -> Self {
        Self { eth_api, token_store }
    }
}

#[async_trait::async_trait]
impl<EthApi: RpcNodeCore> MagnusTokenApiServer for MagnusToken<EthApi> {
    async fn role_history(
        &self,
        _params: PaginationParams<RoleHistoryFilters>,
    ) -> RpcResult<RoleHistoryResponse> {
        Err(internal_rpc_err("unimplemented"))
    }

    async fn tokens(&self, _params: PaginationParams<TokensFilters>) -> RpcResult<TokensResponse> {
        Err(internal_rpc_err("unimplemented"))
    }

    async fn tokens_by_address(
        &self,
        _params: TokensByAddressParams,
    ) -> RpcResult<TokensByAddressResponse> {
        Err(internal_rpc_err("unimplemented"))
    }
}

impl<EthApi: RpcNodeCore> MagnusToken<EthApi> {
    /// Access the underlying provider.
    pub fn provider(&self) -> &EthApi::Provider {
        self.eth_api.provider()
    }
}
