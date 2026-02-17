use crate::rpc::token::{
    role_history::{RoleChange, RoleHistoryFilters, RoleHistoryResponse},
    tokens::{Token, TokensFilters, TokensResponse},
    tokens_by_address::{AccountToken, TokensByAddressParams, TokensByAddressResponse},
};
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use magnus_indexer::TokenStore;
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
        params: PaginationParams<RoleHistoryFilters>,
    ) -> RpcResult<RoleHistoryResponse> {
        let limit = params.limit.unwrap_or(10).min(100);
        let filters = params.filters.unwrap_or_default();
        let (changes, next_cursor) = self.token_store.get_role_history(
            filters.account,
            filters.token,
            filters.role,
            filters.granted,
            filters.sender,
            params.cursor.as_deref(),
            limit,
        );

        Ok(RoleHistoryResponse {
            next_cursor,
            role_changes: changes
                .into_iter()
                .map(|c| RoleChange {
                    account: c.account,
                    block_number: c.block_number,
                    granted: c.granted,
                    role: c.role,
                    sender: c.sender,
                    timestamp: c.timestamp,
                    token: c.token,
                    transaction_hash: c.transaction_hash,
                })
                .collect(),
        })
    }

    async fn tokens(&self, params: PaginationParams<TokensFilters>) -> RpcResult<TokensResponse> {
        let limit = params.limit.unwrap_or(10).min(100);
        let filters = params.filters.unwrap_or_default();
        let (tokens, next_cursor) = self.token_store.get_tokens(
            filters.currency.as_deref(),
            filters.creator,
            filters.paused,
            filters.name.as_deref(),
            filters.symbol.as_deref(),
            params.cursor.as_deref(),
            limit,
        );

        Ok(TokensResponse {
            next_cursor,
            tokens: tokens.into_iter().map(indexed_to_rpc_token).collect(),
        })
    }

    async fn tokens_by_address(
        &self,
        params: TokensByAddressParams,
    ) -> RpcResult<TokensByAddressResponse> {
        let limit = params.params.limit.unwrap_or(10).min(100);
        let currency = params
            .params
            .filters
            .as_ref()
            .and_then(|f| f.currency.clone());
        let (results, next_cursor) = self.token_store.get_tokens_by_address(
            params.address,
            currency.as_deref(),
            params.params.cursor.as_deref(),
            limit,
        );

        Ok(TokensByAddressResponse {
            next_cursor,
            tokens: results
                .into_iter()
                .map(|(token, balance, roles)| AccountToken {
                    balance,
                    roles,
                    token: indexed_to_rpc_token(token),
                })
                .collect(),
        })
    }
}

impl<EthApi: RpcNodeCore> MagnusToken<EthApi> {
    /// Access the underlying provider.
    pub fn provider(&self) -> &EthApi::Provider {
        self.eth_api.provider()
    }
}

fn indexed_to_rpc_token(t: magnus_indexer::IndexedToken) -> Token {
    Token {
        address: t.address,
        created_at: t.created_at,
        creator: t.creator,
        currency: t.currency,
        decimals: t.decimals,
        name: t.name,
        paused: t.paused,
        quote_token: t.quote_token,
        supply_cap: t.supply_cap,
        symbol: t.symbol,
        token_id: t.token_id,
        total_supply: t.total_supply,
        transfer_policy_id: t.transfer_policy_id,
    }
}
