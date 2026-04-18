use crate::{
    MagnusPayloadTypes,
    engine::MagnusEngineValidator,
    rpc::{
        MagnusAdminApi, MagnusAdminApiServer, MagnusEthApi, MagnusEthApiBuilder, MagnusEthExt,
        MagnusEthExtApiServer, MagnusForkScheduleApiServer, MagnusForkScheduleRpc,
        MagnusOperatorApiServer, MagnusOperatorRpc, MagnusSimulate, MagnusSimulateApiServer,
        MagnusToken, MagnusTokenApiServer,
    },
};
use alloy_primitives::B256;
use reth_evm::revm::primitives::Address;
use reth_node_api::{
    AddOnsContext, FullNodeComponents, FullNodeTypes, NodeAddOns, NodeTypes,
    PayloadAttributesBuilder, PayloadTypes,
};
use reth_node_builder::{
    BuilderContext, DebugNode, Node, NodeAdapter,
    components::{
        BasicPayloadServiceBuilder, ComponentsBuilder, ConsensusBuilder, ExecutorBuilder,
        PayloadBuilderBuilder, PoolBuilder, TxPoolBuilder, spawn_maintenance_tasks,
    },
    rpc::{
        BasicEngineValidatorBuilder, EngineValidatorAddOn, NoopEngineApiBuilder,
        PayloadValidatorBuilder, RethRpcAddOns, RpcAddOns, RpcHandle, RpcHooks,
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
use std::default::Default;
use magnus_chainspec::spec::MagnusChainSpec;
use magnus_consensus::MagnusConsensus;
use magnus_evm::MagnusEvmConfig;
use magnus_payload_builder::MagnusPayloadBuilder;
use magnus_payload_types::MagnusPayloadAttributes;
use magnus_primitives::{MagnusHeader, MagnusPrimitives, MagnusTxEnvelope, MagnusTxType};
use magnus_transaction_pool::{
    AA2dPool, AA2dPoolConfig, MagnusTransactionPool,
    amm::AmmLiquidityCache,
    validator::{
        DEFAULT_AA_VALID_AFTER_MAX_SECS, DEFAULT_MAX_MAGNUS_AUTHORIZATIONS,
        MagnusTransactionValidator,
    },
};

/// Magnus node CLI arguments.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::Args)]
pub struct MagnusNodeArgs {
    /// Maximum allowed `valid_after` offset for AA txs.
    #[arg(long = "txpool.aa-valid-after-max-secs", default_value_t = DEFAULT_AA_VALID_AFTER_MAX_SECS)]
    pub aa_valid_after_max_secs: u64,

    /// Maximum number of authorizations allowed in an AA transaction.
    #[arg(long = "txpool.max-magnus-authorizations", default_value_t = DEFAULT_MAX_MAGNUS_AUTHORIZATIONS)]
    pub max_magnus_authorizations: usize,

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
            max_magnus_authorizations: self.max_magnus_authorizations,
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
pub struct MagnusAddOns<N: FullNodeTypes<Types = MagnusNode>> {
    inner: RpcAddOns<
        NodeAdapter<N>,
        MagnusEthApiBuilder,
        MagnusEngineValidatorBuilder,
        NoopEngineApiBuilder,
        BasicEngineValidatorBuilder<MagnusEngineValidatorBuilder>,
        Identity,
    >,
    validator_key: Option<B256>,
}

impl<N> MagnusAddOns<N>
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

impl<N> NodeAddOns<NodeAdapter<N>> for MagnusAddOns<N>
where
    N: FullNodeTypes<Types = MagnusNode>,
{
    type Handle = RpcHandle<NodeAdapter<N>, MagnusEthApi<N>>;

    async fn launch_add_ons(
        self,
        ctx: AddOnsContext<'_, NodeAdapter<N>>,
    ) -> eyre::Result<Self::Handle> {
        let eth_config = EthConfigHandler::new(
            ctx.node.provider.clone(),
            ctx.node.components.evm_config.clone(),
        );

        self.inner
            .launch_add_ons_with(ctx, move |container| {
                let reth_node_builder::rpc::RpcModuleContainer {
                    modules, registry, ..
                } = container;

                let eth_api = registry.eth_api().clone();
                let token = MagnusToken::new(eth_api.clone());
                let eth_ext = MagnusEthExt::new(eth_api.clone());
                let simulate = MagnusSimulate::new(eth_api);
                let admin = MagnusAdminApi::new(self.validator_key);
                let operator = MagnusOperatorRpc::new(registry.admin_api());
                let fork_schedule =
                    MagnusForkScheduleRpc::new(registry.eth_api().provider().clone());

                modules.merge_configured(token.into_rpc())?;
                modules.merge_configured(eth_ext.into_rpc())?;
                modules.merge_if_module_configured(RethRpcModule::Eth, simulate.into_rpc())?;
                modules.merge_configured(fork_schedule.into_rpc())?;
                modules.merge_if_module_configured(
                    RethRpcModule::Other("operator".to_string()),
                    operator.into_rpc(),
                )?;
                modules.merge_if_module_configured(RethRpcModule::Admin, admin.into_rpc())?;
                modules.merge_if_module_configured(RethRpcModule::Eth, eth_config.into_rpc())?;

                Ok(())
            })
            .await
    }
}

impl<N> RethRpcAddOns<NodeAdapter<N>> for MagnusAddOns<N>
where
    N: FullNodeTypes<Types = MagnusNode>,
{
    type EthApi = MagnusEthApi<N>;

    fn hooks_mut(&mut self) -> &mut RpcHooks<NodeAdapter<N>, Self::EthApi> {
        self.inner.hooks_mut()
    }
}

impl<N> EngineValidatorAddOn<NodeAdapter<N>> for MagnusAddOns<N>
where
    N: FullNodeTypes<Types = MagnusNode>,
{
    type ValidatorBuilder = BasicEngineValidatorBuilder<MagnusEngineValidatorBuilder>;

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

    type AddOns = MagnusAddOns<N>;

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
        _chain_spec: &Self::ChainSpec,
    ) -> impl PayloadAttributesBuilder<<Self::Payload as PayloadTypes>::PayloadAttributes, MagnusHeader>
    {
        MagnusPayloadAttributesBuilder::new()
    }
}

/// The attributes builder with a restricted set of validators
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct MagnusPayloadAttributesBuilder;

impl MagnusPayloadAttributesBuilder {
    /// Creates a new instance of the builder.
    pub const fn new() -> Self {
        Self
    }
}

impl PayloadAttributesBuilder<MagnusPayloadAttributes, MagnusHeader>
    for MagnusPayloadAttributesBuilder
{
    fn build(&self, _parent: &SealedHeader<MagnusHeader>) -> MagnusPayloadAttributes {
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let (timestamp, timestamp_millis_part) = (millis / 1000, millis % 1000);
        MagnusPayloadAttributes::new(
            Address::ZERO,
            None,
            timestamp,
            timestamp_millis_part,
            Default::default(),
            None,
            Vec::new,
        )
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
        let evm_config = MagnusEvmConfig::new(ctx.chain_spec());
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
    /// Maximum number of authorizations allowed in an AA transaction.
    pub max_magnus_authorizations: usize,
}

impl MagnusPoolBuilder {
    /// Sets the maximum allowed `valid_after` offset for AA txs.
    pub const fn with_aa_tx_valid_after_max_secs(mut self, secs: u64) -> Self {
        self.aa_valid_after_max_secs = secs;
        self
    }

    /// Sets the maximum number of authorizations allowed in an AA transaction.
    pub const fn with_max_magnus_authorizations(mut self, max: usize) -> Self {
        self.max_magnus_authorizations = max;
        self
    }
}

impl Default for MagnusPoolBuilder {
    fn default() -> Self {
        Self {
            aa_valid_after_max_secs: DEFAULT_AA_VALID_AFTER_MAX_SECS,
            max_magnus_authorizations: DEFAULT_MAX_MAGNUS_AUTHORIZATIONS,
        }
    }
}

impl<Node> PoolBuilder<Node, MagnusEvmConfig> for MagnusPoolBuilder
where
    Node: FullNodeTypes<Types = MagnusNode>,
{
    type Pool = MagnusTransactionPool<Node::Provider>;

    async fn build_pool(
        self,
        ctx: &BuilderContext<Node>,
        evm_config: MagnusEvmConfig,
    ) -> eyre::Result<Self::Pool> {
        let mut pool_config = ctx.pool_config();
        pool_config.max_inflight_delegated_slot_limit = pool_config.max_account_slots;

        // this store is effectively a noop
        let blob_store = InMemoryBlobStore::default();
        let validator =
            TransactionValidationTaskExecutor::eth_builder(ctx.provider().clone(), evm_config)
                .with_max_tx_input_bytes(ctx.config().txpool.max_tx_input_bytes)
                .with_local_transactions_config(pool_config.local_transactions_config.clone())
                .set_tx_fee_cap(ctx.config().rpc.rpc_tx_fee_cap)
                .with_max_tx_gas_limit(ctx.config().txpool.max_tx_gas_limit)
                .set_block_gas_limit(ctx.chain_spec().inner.genesis().gas_limit)
                .disable_balance_check()
                .with_minimum_priority_fee(ctx.config().txpool.minimum_priority_fee)
                .with_additional_tasks(ctx.config().txpool.additional_validation_tasks)
                .with_custom_tx_type(MagnusTxType::AA as u8)
                .no_eip4844()
                .build_with_tasks(ctx.task_executor().clone(), blob_store.clone());

        let aa_2d_config = AA2dPoolConfig {
            price_bump_config: pool_config.price_bumps,
            pending_limit: pool_config.pending_limit,
            queued_limit: pool_config.queued_limit,
            max_txs_per_sender: pool_config.max_account_slots,
        };
        let aa_2d_pool = AA2dPool::new(aa_2d_config);
        let amm_liquidity_cache = AmmLiquidityCache::new(ctx.provider())?;

        let validator = validator.map(|v| {
            MagnusTransactionValidator::new(
                v,
                self.aa_valid_after_max_secs,
                self.max_magnus_authorizations,
                amm_liquidity_cache.clone(),
            )
        });
        let protocol_pool = TxPoolBuilder::new(ctx)
            .with_validator(validator)
            .build(blob_store, pool_config.clone());

        // Wrap the protocol pool in our hybrid MagnusTransactionPool
        let transaction_pool = MagnusTransactionPool::new(protocol_pool, aa_2d_pool);

        spawn_maintenance_tasks(ctx, transaction_pool.clone(), &pool_config)?;

        // Spawn unified Magnus pool maintenance task
        // This consolidates: expired AA txs, 2D nonce updates, AMM cache, and keychain revocations
        ctx.task_executor().spawn_critical_task(
            "txpool maintenance - magnus pool",
            magnus_transaction_pool::maintain::maintain_magnus_pool(transaction_pool.clone()),
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
            ctx.is_dev(),
            self.state_provider_metrics,
            self.disable_state_cache,
        ))
    }
}
