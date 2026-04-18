use crate::{node::MagnusNode, rpc::MagnusEthApi};
use alloy_primitives::{Address, B256, keccak256};
use alloy_rpc_types_eth::simulate::SimulatedBlock;
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use reth_ethereum::evm::revm::database::StateProviderDatabase;
use reth_node_api::FullNodeTypes;
use reth_primitives_traits::AlloyBlockHeader as _;
use reth_provider::{BlockIdReader, ChainSpecProvider, HeaderProvider};
use reth_rpc_eth_api::{
    RpcBlock, RpcNodeCore,
    helpers::{EthCall, LoadState, SpawnBlocking},
};
use reth_tracing::tracing;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    sync::LazyLock,
};
use magnus_chainspec::hardfork::MagnusHardforks;
use magnus_evm::MagnusStateAccess;
use magnus_precompiles::{error::MagnusPrecompileError, mip20::MIP20Token};
use magnus_primitives::MagnusAddressExt;

/// keccak256("Transfer(address,address,uint256)")
static TRANSFER_TOPIC: LazyLock<B256> =
    LazyLock::new(|| keccak256(b"Transfer(address,address,uint256)"));

/// MIP-20 token metadata returned alongside simulation results.
///
/// `decimals` is omitted because all MIP-20 tokens use a fixed decimal count.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tip20TokenMetadata {
    pub name: String,
    pub symbol: String,
    pub currency: String,
}

/// Response for `magnus_simulateV1`.
///
/// Wraps the standard `eth_simulateV1` response with a top-level `tokenMetadata` map
/// containing MIP-20 token info for all tokens involved in transfer logs.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MagnusSimulateV1Response<B> {
    /// Standard simulation results (one per simulated block).
    pub blocks: Vec<SimulatedBlock<B>>,
    /// Token metadata for MIP-20 addresses that appear in Transfer logs.
    pub token_metadata: BTreeMap<Address, Tip20TokenMetadata>,
}

#[rpc(server, namespace = "magnus")]
pub trait MagnusSimulateApi {
    /// Simulates transactions like `eth_simulateV1` but enriches the response with
    /// MIP-20 token metadata for all tokens involved in Transfer events.
    ///
    /// This eliminates the need for a second roundtrip to fetch token symbols/decimals
    /// after simulation.
    #[method(name = "simulateV1")]
    async fn simulate_v1(
        &self,
        payload: alloy_rpc_types_eth::simulate::SimulatePayload<
            magnus_alloy::rpc::MagnusTransactionRequest,
        >,
        block: Option<alloy_eips::BlockId>,
    ) -> RpcResult<MagnusSimulateV1Response<RpcBlock<magnus_alloy::MagnusNetwork>>>;
}

/// Implementation of `magnus_simulateV1`.
#[derive(Debug, Clone)]
pub struct MagnusSimulate<N: FullNodeTypes<Types = MagnusNode>> {
    eth_api: MagnusEthApi<N>,
}

impl<N: FullNodeTypes<Types = MagnusNode>> MagnusSimulate<N> {
    pub fn new(eth_api: MagnusEthApi<N>) -> Self {
        Self { eth_api }
    }
}

/// Extract MIP-20 addresses from the simulation request's call targets.
///
/// This allows metadata resolution to start before simulation completes.
fn extract_tip20_targets(
    payload: &alloy_rpc_types_eth::simulate::SimulatePayload<
        magnus_alloy::rpc::MagnusTransactionRequest,
    >,
) -> Vec<Address> {
    let mut addrs = std::collections::BTreeSet::new();
    for block in &payload.block_state_calls {
        for call in &block.calls {
            // Standard `to` field
            if let Some(to) = call.to.as_ref().and_then(|k| k.to())
                && to.is_tip20()
            {
                addrs.insert(*to);
            }
            // AA calls array
            for c in &call.calls {
                if let Some(to) = c.to.to()
                    && to.is_tip20()
                {
                    addrs.insert(*to);
                }
            }
            // Fee token
            if let Some(ft) = call.fee_token
                && ft.is_tip20()
            {
                addrs.insert(ft);
            }
        }
    }
    addrs.into_iter().collect()
}

#[async_trait::async_trait]
impl<N: FullNodeTypes<Types = MagnusNode>> MagnusSimulateApiServer for MagnusSimulate<N> {
    async fn simulate_v1(
        &self,
        payload: alloy_rpc_types_eth::simulate::SimulatePayload<
            magnus_alloy::rpc::MagnusTransactionRequest,
        >,
        block: Option<alloy_eips::BlockId>,
    ) -> RpcResult<MagnusSimulateV1Response<RpcBlock<magnus_alloy::MagnusNetwork>>> {
        // Pre-extract MIP-20 addresses from call targets so we can start
        // metadata resolution concurrently with the simulation.
        let prefetched = extract_tip20_targets(&payload);

        // Run simulation and metadata prefetch concurrently
        let (sim_result, mut token_metadata) = tokio::join!(
            self.eth_api.simulate_v1(payload, block),
            self.resolve_token_metadata(prefetched, block),
        );

        let blocks = sim_result.map_err(|e| {
            let err: jsonrpsee::types::ErrorObject<'static> = e.into();
            err
        })?;

        // Scan simulation logs for any additional MIP-20 addresses not in the
        // prefetched set (e.g. tokens touched indirectly via contract calls).
        let mut extra = HashSet::new();
        for sim_block in &blocks {
            for call in &sim_block.calls {
                for log in &call.logs {
                    if log.address().is_tip20()
                        && log.topics().first() == Some(&*TRANSFER_TOPIC)
                        && !token_metadata.contains_key(&log.address())
                    {
                        extra.insert(log.address());
                    }
                }
            }
        }

        if !extra.is_empty() {
            let extra_metadata = self
                .resolve_token_metadata(extra.into_iter().collect(), block)
                .await;
            token_metadata.extend(extra_metadata);
        }

        Ok(MagnusSimulateV1Response {
            blocks,
            token_metadata,
        })
    }
}

impl<N: FullNodeTypes<Types = MagnusNode>> MagnusSimulate<N> {
    /// Resolves MIP-20 token metadata for the given addresses using state at the target block.
    async fn resolve_token_metadata(
        &self,
        addresses: Vec<Address>,
        block: Option<alloy_eips::BlockId>,
    ) -> BTreeMap<Address, Tip20TokenMetadata> {
        if addresses.is_empty() {
            return BTreeMap::new();
        }

        let result = self
            .eth_api
            .spawn_blocking_io_fut(async move |this| {
                let state = this.state_at_block_id_or_latest(block).await?;

                // Derive hardfork spec from the target block's timestamp.
                let timestamp = block
                    .and_then(|id| {
                        this.provider()
                            .block_number_for_id(id)
                            .ok()
                            .flatten()
                            .and_then(|num| {
                                this.provider()
                                    .header_by_number(num)
                                    .ok()
                                    .flatten()
                                    .map(|h| h.timestamp())
                            })
                    })
                    .unwrap_or(u64::MAX);

                let spec = this.provider().chain_spec().magnus_hardfork_at(timestamp);
                let mut db = StateProviderDatabase::new(state);

                let mut metadata = BTreeMap::new();
                for addr in &addresses {
                    let result = db.with_read_only_storage_ctx(spec, || {
                        let token = MIP20Token::from_address(*addr)?;
                        Ok::<_, MagnusPrecompileError>((
                            token.name()?,
                            token.symbol()?,
                            token.currency()?,
                        ))
                    });

                    match result {
                        Ok((name, symbol, currency)) => {
                            metadata.insert(
                                *addr,
                                Tip20TokenMetadata {
                                    name,
                                    symbol,
                                    currency,
                                },
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                token = %addr,
                                error = %e,
                                "failed to resolve MIP-20 metadata, skipping"
                            );
                        }
                    }
                }

                Ok(metadata)
            })
            .await;

        match result {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(error = ?e, "failed to resolve token metadata");
                BTreeMap::new()
            }
        }
    }
}
