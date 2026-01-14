use crate::{
    MagnusPayloadTypes,
    engine::MagnusEngineValidator,
    rpc::{
        MagnusAdminApi, MagnusAdminApiServer, MagnusEthApiBuilder, MagnusEthExt, MagnusEthExtApiServer,
        MagnusToken, MagnusTokenApiServer,
    },
};
use alloy_primitives::B256;
use reth_engine_local::LocalPayloadAttributesBuilder;
use reth_evm::revm::primitives::Address;
use reth_node_api::{
    AddOnsContext, FullNodeComponents, FullNodeTypes, NodeAddOns, NodePrimitives, NodeTypes,
    PayloadAttributesBuilder, PayloadTypes,
};
use reth_node_builder::{
    BuilderContext, DebugNode, Node, NodeAdapter,
    components::{
        BasicPayloadServiceBuilder, ComponentsBuilder, ConsensusBuilder, ExecutorBuilder,
        PayloadBuilderBuilder, PoolBuilder, TxPoolBuilder, spawn_maintenance_tasks,
    },
    rpc::{
        BasicEngineValidatorBuilder, EngineValidatorAddOn, EngineValidatorBuilder, EthApiBuilder,
        NoopEngineApiBuilder, PayloadValidatorBuilder, RethRpcAddOns, RpcAddOns,
    },
};
use reth_node_ethereum::EthereumNetworkBuilder;
use reth_primitives_traits::SealedHeader;
use reth_provider::{EthStorage, providers::ProviderFactoryBuilder};
use reth_rpc_builder::{Identity, RethRpcModule};
use reth_rpc_eth_api::{
    RpcNodeCore,
    helpers::config::{EthConfigApiServer, EthConfigHandler},
};
use reth_tracing::tracing::{debug, info};
use reth_transaction_pool::{TransactionValidationTaskExecutor, blobstore::InMemoryBlobStore};
use std::{default::Default, sync::Arc};
use magnus_chainspec::spec::{MAGNUS_BASE_FEE, MagnusChainSpec};
use magnus_consensus::MagnusConsensus;
use magnus_evm::{MagnusEvmConfig, evm::MagnusEvmFactory};
use magnus_payload::MagnusPayloadBuilder;
use magnus_payload_types::MagnusPayloadAttributes;
use magnus_primitives::{MagnusHeader, MagnusPrimitives, MagnusTxEnvelope, MagnusTxType};
use magnus_mempool::{
    AA2dPool, AA2dPoolConfig, MagnusTransactionPool, amm::AmmLiquidityCache,
    validator::MagnusTransactionValidator,
};

/// Default maximum allowed `valid_after` offset for AA txs (1 hour).
pub const DEFAULT_AA_VALID_AFTER_MAX_SECS: u64 = 3600;

/// Magnus node CLI arguments.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::Args)]
pub struct MagnusNodeArgs {
    /// Maximum allowed `valid_after` offset for AA txs.
    #[arg(long = "txpool.aa-valid-after-max-secs", default_value_t = DEFAULT_AA_VALID_AFTER_MAX_SECS)]
    pub aa_valid_after_max_secs: u64,

    /// Enable state provider metrics for the payload builder.
    #[arg(long = "builder.state-provider-metrics", default_value_t = false)]
    pub builder_state_provider_metrics: bool,

    /// Disable state cache for the payload builder.
    #[arg(long = "builder.disable-state-cache", default_value_t = false)]
    pub builder_disable_state_cache: bool,
}

impl MagnusNodeArgs {
    /// Returns a [`MagnusPoolBuilder`] configured from these args.
    pub fn pool_builder(&self) -> MagnusPoolBuilder {
        MagnusPoolBuilder {
            aa_valid_after_max_secs: self.aa_valid_after_max_secs,
        }
    }

    /// Returns a [`MagnusPayloadBuilderBuilder`] configured from these args.
    pub fn payload_builder_builder(&self) -> MagnusPayloadBuilderBuilder {
        MagnusPayloadBuilderBuilder {
            state_provider_metrics: self.builder_state_provider_metrics,
            disable_state_cache: self.builder_disable_state_cache,
        }
    }
}

/// Type configuration for a regular Ethereum node.
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct MagnusNode {
    /// Transaction pool builder.
    pool_builder: MagnusPoolBuilder,
    /// Payload builder builder.
    payload_builder_builder: MagnusPayloadBuilderBuilder,
    /// Validator public key for `admin_validatorKey` RPC method.
    validator_key: Option<B256>,
}

impl MagnusNode {
    /// Create new instance of a Magnus node
    pub fn new(args: &MagnusNodeArgs, validator_key: Option<B256>) -> Self {
        Self {
            pool_builder: args.pool_builder(),
            payload_builder_builder: args.payload_builder_builder(),
            validator_key,
        }
    }

    /// Returns a [`ComponentsBuilder`] configured for a regular Magnus node.
    pub fn components<Node>(
        pool_builder: MagnusPoolBuilder,
        payload_builder_builder: MagnusPayloadBuilderBuilder,
    ) -> ComponentsBuilder<
        Node,
        MagnusPoolBuilder,
        BasicPayloadServiceBuilder<MagnusPayloadBuilderBuilder>,
        EthereumNetworkBuilder,
        MagnusExecutorBuilder,
        MagnusConsensusBuilder,
    >
    where
        Node: FullNodeTypes<Types = Self>,
    {
        ComponentsBuilder::default()
            .node_types::<Node>()
            .pool(pool_builder)
            .executor(MagnusExecutorBuilder::default())
            .payload(BasicPayloadServiceBuilder::new(payload_builder_builder))
            .network(EthereumNetworkBuilder::default())
            .consensus(MagnusConsensusBuilder::default())
    }

    pub fn provider_factory_builder() -> ProviderFactoryBuilder<Self> {
        ProviderFactoryBuilder::default()
    }

    /// Sets the validator key for filtering subblock transactions.
    pub fn with_validator_key(mut self, validator_key: Option<B256>) -> Self {
        self.validator_key = validator_key;
        self
    }
}

impl NodeTypes for MagnusNode {
    type Primitives = MagnusPrimitives;
    type ChainSpec = MagnusChainSpec;
    type Storage = EthStorage<MagnusTxEnvelope, MagnusHeader>;
    type Payload = MagnusPayloadTypes;
}

#[derive(Debug)]
pub struct MagnusAddOns<
    N: FullNodeComponents,
    EthB: EthApiBuilder<N> = MagnusEthApiBuilder,
    PVB = MagnusEngineValidatorBuilder,
    EVB = BasicEngineValidatorBuilder<PVB>,
    RpcMiddleware = Identity,
> {
    inner: RpcAddOns<N, EthB, PVB, NoopEngineApiBuilder, EVB, RpcMiddleware>,
    validator_key: Option<B256>,
}

impl<N> MagnusAddOns<NodeAdapter<N>, MagnusEthApiBuilder>
where
    N: FullNodeTypes<Types = MagnusNode>,
{
    /// Creates a new instance from the inner `RpcAddOns`.
    pub fn new(validator_key: Option<B256>) -> Self {
        Self {
            inner: RpcAddOns::new(
                MagnusEthApiBuilder::new(validator_key),
                MagnusEngineValidatorBuilder,
                NoopEngineApiBuilder::default(),
                BasicEngineValidatorBuilder::default(),
                Identity::default(),
            ),
            validator_key,
        }
    }
}

impl<N, EthB, PVB, EVB> NodeAddOns<N> for MagnusAddOns<N, EthB, PVB, EVB>
where
    N: FullNodeComponents<Types = MagnusNode, Evm = MagnusEvmConfig>,
    EthB: EthApiBuilder<N>,
    PVB: Send + PayloadValidatorBuilder<N>,
    EVB: EngineValidatorBuilder<N>,
    EthB::EthApi:
        RpcNodeCore<Evm = MagnusEvmConfig, Primitives: NodePrimitives<BlockHeader = MagnusHeader>>,
{
    type Handle = <RpcAddOns<N, EthB, PVB, NoopEngineApiBuilder, EVB> as NodeAddOns<N>>::Handle;

    async fn launch_add_ons(self, ctx: AddOnsContext<'_, N>) -> eyre::Result<Self::Handle> {
        let eth_config =
            EthConfigHandler::new(ctx.node.provider().clone(), ctx.node.evm_config().clone());

        self.inner
            .launch_add_ons_with(ctx, move |container| {
                let reth_node_builder::rpc::RpcModuleContainer {
                    modules, registry, ..
                } = container;

                let eth_api = registry.eth_api().clone();
                let token = MagnusToken::new(eth_api.clone());
                let eth_ext = MagnusEthExt::new(eth_api);
                let admin = MagnusAdminApi::new(self.validator_key);

                modules.merge_configured(token.into_rpc())?;
                modules.merge_configured(eth_ext.into_rpc())?;
                modules.merge_if_module_configured(RethRpcModule::Admin, admin.into_rpc())?;
                modules.merge_if_module_configured(RethRpcModule::Eth, eth_config.into_rpc())?;

                Ok(())
            })
            .await
    }
}

impl<N, EthB, PVB, EVB> RethRpcAddOns<N> for MagnusAddOns<N, EthB, PVB, EVB>
where
    N: FullNodeComponents<Types = MagnusNode, Evm = MagnusEvmConfig>,
    EthB: EthApiBuilder<N>,
    PVB: PayloadValidatorBuilder<N>,
    EVB: EngineValidatorBuilder<N>,
    EthB::EthApi:
        RpcNodeCore<Evm = MagnusEvmConfig, Primitives: NodePrimitives<BlockHeader = MagnusHeader>>,
{
    type EthApi = EthB::EthApi;

    fn hooks_mut(&mut self) -> &mut reth_node_builder::rpc::RpcHooks<N, Self::EthApi> {
        self.inner.hooks_mut()
    }
}

impl<N, EthB, PVB, EVB> EngineValidatorAddOn<N> for MagnusAddOns<N, EthB, PVB, EVB>
where
    N: FullNodeComponents<Types = MagnusNode, Evm = MagnusEvmConfig>,
    EthB: EthApiBuilder<N>,
    PVB: Send,
    EVB: EngineValidatorBuilder<N>,
{
    type ValidatorBuilder = EVB;

    fn engine_validator_builder(&self) -> Self::ValidatorBuilder {
        self.inner.engine_validator_builder()
    }
}

impl<N> Node<N> for MagnusNode
where
    N: FullNodeTypes<Types = Self>,
{
    type ComponentsBuilder = ComponentsBuilder<
        N,
        MagnusPoolBuilder,
        BasicPayloadServiceBuilder<MagnusPayloadBuilderBuilder>,
        EthereumNetworkBuilder,
        MagnusExecutorBuilder,
        MagnusConsensusBuilder,
    >;

    type AddOns = MagnusAddOns<NodeAdapter<N>>;

    fn components_builder(&self) -> Self::ComponentsBuilder {
        Self::components(self.pool_builder, self.payload_builder_builder)
    }

    fn add_ons(&self) -> Self::AddOns {
        MagnusAddOns::new(self.validator_key)
    }
}

impl<N: FullNodeComponents<Types = Self>> DebugNode<N> for MagnusNode {
    type RpcBlock =
        alloy_rpc_types_eth::Block<alloy_rpc_types_eth::Transaction<MagnusTxEnvelope>, MagnusHeader>;

    fn rpc_to_primitive_block(rpc_block: Self::RpcBlock) -> magnus_primitives::Block {
        rpc_block
            .into_consensus_block()
            .map_transactions(|tx| tx.into_inner())
    }

    fn local_payload_attributes_builder(
        chain_spec: &Self::ChainSpec,
    ) -> impl PayloadAttributesBuilder<<Self::Payload as PayloadTypes>::PayloadAttributes, MagnusHeader>
    {
        MagnusPayloadAttributesBuilder::new(Arc::new(chain_spec.clone()))
    }
}

/// The attributes builder with a restricted set of validators
#[derive(Debug)]
#[non_exhaustive]
pub struct MagnusPayloadAttributesBuilder {
    /// The vanilla eth payload attributes builder
    inner: LocalPayloadAttributesBuilder<MagnusChainSpec>,
}

impl MagnusPayloadAttributesBuilder {
    /// Creates a new instance of the builder.
    pub fn new(chain_spec: Arc<MagnusChainSpec>) -> Self {
        Self {
            inner: LocalPayloadAttributesBuilder::new(chain_spec).without_increasing_timestamp(),
        }
    }
}

impl PayloadAttributesBuilder<MagnusPayloadAttributes, MagnusHeader>
    for MagnusPayloadAttributesBuilder
{
    fn build(&self, parent: &SealedHeader<MagnusHeader>) -> MagnusPayloadAttributes {
        let mut inner = self.inner.build(parent);
        inner.suggested_fee_recipient = Address::ZERO;

        let timestamp_millis_part = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
            % 1000;

        MagnusPayloadAttributes {
            inner,
            timestamp_millis_part,
        }
    }
}

/// A regular ethereum evm and executor builder.
#[derive(Debug, Default, Clone, Copy)]
#[non_exhaustive]
pub struct MagnusExecutorBuilder;

impl<Node> ExecutorBuilder<Node> for MagnusExecutorBuilder
where
    Node: FullNodeTypes<Types = MagnusNode>,
{
    type EVM = MagnusEvmConfig;

    async fn build_evm(self, ctx: &BuilderContext<Node>) -> eyre::Result<Self::EVM> {
        let evm_config = MagnusEvmConfig::new(ctx.chain_spec(), MagnusEvmFactory::default());
        Ok(evm_config)
    }
}

/// Builder for [`MagnusConsensus`].
#[derive(Debug, Default, Clone, Copy)]
#[non_exhaustive]
pub struct MagnusConsensusBuilder;

impl<Node> ConsensusBuilder<Node> for MagnusConsensusBuilder
where
    Node: FullNodeTypes<Types = MagnusNode>,
{
    type Consensus = MagnusConsensus;

    async fn build_consensus(self, ctx: &BuilderContext<Node>) -> eyre::Result<Self::Consensus> {
        Ok(MagnusConsensus::new(ctx.chain_spec()))
    }
}

/// Builder for [`MagnusEngineValidator`].
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct MagnusEngineValidatorBuilder;

impl<Node> PayloadValidatorBuilder<Node> for MagnusEngineValidatorBuilder
where
    Node: FullNodeComponents<Types = MagnusNode>,
{
    type Validator = MagnusEngineValidator;

    async fn build(self, _ctx: &AddOnsContext<'_, Node>) -> eyre::Result<Self::Validator> {
        Ok(MagnusEngineValidator::new())
    }
}

/// A basic Magnus transaction pool.
///
/// This contains various settings that can be configured and take precedence over the node's
/// config.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct MagnusPoolBuilder {
    /// Maximum allowed `valid_after` offset for AA txs.
    pub aa_valid_after_max_secs: u64,
}

impl MagnusPoolBuilder {
    /// Sets the maximum allowed `valid_after` offset for AA txs.
    pub const fn with_aa_tx_valid_after_max_secs(mut self, secs: u64) -> Self {
        self.aa_valid_after_max_secs = secs;
        self
    }
}

impl Default for MagnusPoolBuilder {
    fn default() -> Self {
        Self {
            aa_valid_after_max_secs: DEFAULT_AA_VALID_AFTER_MAX_SECS,
        }
    }
}

impl<Node> PoolBuilder<Node> for MagnusPoolBuilder
where
    Node: FullNodeTypes<Types = MagnusNode>,
{
    type Pool = MagnusTransactionPool<Node::Provider>;

    async fn build_pool(self, ctx: &BuilderContext<Node>) -> eyre::Result<Self::Pool> {
        let mut pool_config = ctx.pool_config();
        pool_config.minimal_protocol_basefee = MAGNUS_BASE_FEE;
        pool_config.max_inflight_delegated_slot_limit = pool_config.max_account_slots;

        // this store is effectively a noop
        let blob_store = InMemoryBlobStore::default();
        let validator = TransactionValidationTaskExecutor::eth_builder(ctx.provider().clone())
            .with_head_timestamp(ctx.head().timestamp)
            .with_max_tx_input_bytes(ctx.config().txpool.max_tx_input_bytes)
            .with_local_transactions_config(pool_config.local_transactions_config.clone())
            .set_tx_fee_cap(ctx.config().rpc.rpc_tx_fee_cap)
            .with_max_tx_gas_limit(ctx.config().txpool.max_tx_gas_limit)
            .disable_balance_check()
            .with_minimum_priority_fee(ctx.config().txpool.minimum_priority_fee)
            .with_additional_tasks(ctx.config().txpool.additional_validation_tasks)
            .with_custom_tx_type(MagnusTxType::AA as u8)
            .no_eip4844()
            .build_with_tasks(ctx.task_executor().clone(), blob_store.clone());

        let aa_2d_config = AA2dPoolConfig {
            price_bump_config: pool_config.price_bumps,
            // TODO: configure dedicated limit
            aa_2d_limit: pool_config.pending_limit,
        };
        let aa_2d_pool = AA2dPool::new(aa_2d_config);
        let amm_liquidity_cache = AmmLiquidityCache::new(ctx.provider())?;

        let validator = validator.map(|v| {
            MagnusTransactionValidator::new(
                v,
                self.aa_valid_after_max_secs,
                amm_liquidity_cache.clone(),
            )
        });
        let protocol_pool = TxPoolBuilder::new(ctx)
            .with_validator(validator)
            .build(blob_store, pool_config.clone());

        // Wrap the protocol pool in our hybrid MagnusTransactionPool
        let transaction_pool = MagnusTransactionPool::new(protocol_pool, aa_2d_pool);

        spawn_maintenance_tasks(ctx, transaction_pool.clone(), &pool_config)?;

        // Spawn (protocol) mempool maintenance tasks
        let task_pool = transaction_pool.clone();
        let task_provider = ctx.provider().clone();
        ctx.task_executor().spawn_critical(
            "txpool maintenance (protocol) - evict expired AA txs",
            magnus_mempool::maintain::evict_expired_aa_txs(task_pool, task_provider),
        );

        // Spawn (AA 2d nonce) mempool maintenance tasks
        ctx.task_executor().spawn_critical(
            "txpool maintenance - 2d nonce AA txs",
            magnus_mempool::maintain::maintain_2d_nonce_pool(transaction_pool.clone()),
        );

        // Spawn AMM liquidity cache maintenance task
        ctx.task_executor().spawn_critical(
            "txpool maintenance - amm liquidity cache",
            magnus_mempool::maintain::maintain_amm_cache(transaction_pool.clone()),
        );

        info!(target: "reth::cli", "Transaction pool initialized");
        debug!(target: "reth::cli", "Spawned txpool maintenance task");

        Ok(transaction_pool)
    }
}

#[derive(Debug, Default, Clone, Copy)]
#[non_exhaustive]
pub struct MagnusPayloadBuilderBuilder {
    /// Enable state provider metrics for the payload builder.
    pub state_provider_metrics: bool,
    /// Disable state cache for the payload builder.
    pub disable_state_cache: bool,
}

impl<Node> PayloadBuilderBuilder<Node, MagnusTransactionPool<Node::Provider>, MagnusEvmConfig>
    for MagnusPayloadBuilderBuilder
where
    Node: FullNodeTypes<Types = MagnusNode>,
{
    type PayloadBuilder = MagnusPayloadBuilder<Node::Provider>;

    async fn build_payload_builder(
        self,
        ctx: &BuilderContext<Node>,
        pool: MagnusTransactionPool<Node::Provider>,
        evm_config: MagnusEvmConfig,
    ) -> eyre::Result<Self::PayloadBuilder> {
        Ok(MagnusPayloadBuilder::new(
            pool,
            ctx.provider().clone(),
            evm_config,
            self.state_provider_metrics,
            self.disable_state_cache,
        ))
    }
}
