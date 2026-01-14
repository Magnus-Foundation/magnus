use crate::{
    bootnodes::{andantino_nodes, moderato_nodes},
    hardfork::{MagnusHardfork, MagnusHardforks},
};
use alloy_eips::eip7840::BlobParams;
use alloy_evm::eth::spec::EthExecutorSpec;
use alloy_genesis::Genesis;
use alloy_primitives::{Address, B256, U256};
use reth_chainspec::{
    BaseFeeParams, Chain, ChainSpec, DepositContract, DisplayHardforks, EthChainSpec,
    EthereumHardfork, EthereumHardforks, ForkCondition, ForkFilter, ForkId, Hardfork, Hardforks,
    Head,
};
use reth_network_peers::NodeRecord;
use std::sync::{Arc, LazyLock};
use magnus_primitives::MagnusHeader;

pub const MAGNUS_BASE_FEE: u64 = 10_000_000_000;

// End-of-block system transactions
pub const SYSTEM_TX_COUNT: usize = 1;
pub const SYSTEM_TX_ADDRESSES: [Address; SYSTEM_TX_COUNT] = [Address::ZERO];

/// Magnus genesis info extracted from genesis extra_fields
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MagnusGenesisInfo {
    /// The epoch length used by consensus.
    #[serde(skip_serializing_if = "Option::is_none")]
    epoch_length: Option<u64>,
    /// Activation timestamp for T0 hardfork.
    #[serde(skip_serializing_if = "Option::is_none")]
    t0_time: Option<u64>,
}

impl MagnusGenesisInfo {
    /// Extract Magnus genesis info from genesis extra_fields
    fn extract_from(genesis: &Genesis) -> Self {
        genesis
            .config
            .extra_fields
            .deserialize_as::<Self>()
            .unwrap_or_default()
    }

    pub fn epoch_length(&self) -> Option<u64> {
        self.epoch_length
    }

    pub fn t0_time(&self) -> Option<u64> {
        self.t0_time
    }
}

/// Magnus chain specification parser.
#[derive(Debug, Clone, Default)]
pub struct MagnusChainSpecParser;

/// Chains supported by Magnus. First value should be used as the default.
pub const SUPPORTED_CHAINS: &[&str] = &["moderato", "testnet"];

/// Clap value parser for [`ChainSpec`]s.
///
/// The value parser matches either a known chain, the path
/// to a json file, or a json formatted string in-memory. The json needs to be a Genesis struct.
#[cfg(feature = "cli")]
pub fn chain_value_parser(s: &str) -> eyre::Result<Arc<MagnusChainSpec>> {
    Ok(match s {
        "testnet" => ANDANTINO.clone(),
        "moderato" => MODERATO.clone(),
        "dev" => DEV.clone(),
        _ => MagnusChainSpec::from_genesis(reth_cli::chainspec::parse_genesis(s)?).into(),
    })
}

#[cfg(feature = "cli")]
impl reth_cli::chainspec::ChainSpecParser for MagnusChainSpecParser {
    type ChainSpec = MagnusChainSpec;

    const SUPPORTED_CHAINS: &'static [&'static str] = SUPPORTED_CHAINS;

    fn parse(s: &str) -> eyre::Result<Arc<Self::ChainSpec>> {
        chain_value_parser(s)
    }
}

pub static ANDANTINO: LazyLock<Arc<MagnusChainSpec>> = LazyLock::new(|| {
    let genesis: Genesis = serde_json::from_str(include_str!("./genesis/andantino.json"))
        .expect("`./genesis/andantino.json` must be present and deserializable");
    MagnusChainSpec::from_genesis(genesis)
        .with_default_follow_url("wss://rpc.testnet.magnus.network")
        .into()
});

pub static MODERATO: LazyLock<Arc<MagnusChainSpec>> = LazyLock::new(|| {
    let genesis: Genesis = serde_json::from_str(include_str!("./genesis/moderato.json"))
        .expect("`./genesis/moderato.json` must be present and deserializable");
    MagnusChainSpec::from_genesis(genesis)
        .with_default_follow_url("wss://rpc.moderato.magnus.network")
        .into()
});

/// Development chainspec with funded dev accounts and activated magnus hardforks
///
/// `cargo x generate-genesis -o dev.json --accounts 10`
pub static DEV: LazyLock<Arc<MagnusChainSpec>> = LazyLock::new(|| {
    let genesis: Genesis = serde_json::from_str(include_str!("./genesis/dev.json"))
        .expect("`./genesis/dev.json` must be present and deserializable");
    MagnusChainSpec::from_genesis(genesis).into()
});

/// Magnus chain spec type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MagnusChainSpec {
    /// [`ChainSpec`].
    pub inner: ChainSpec<MagnusHeader>,
    pub info: MagnusGenesisInfo,
    /// Default RPC URL for following this chain.
    pub default_follow_url: Option<&'static str>,
}

impl MagnusChainSpec {
    /// Returns the default RPC URL for following this chain.
    pub fn default_follow_url(&self) -> Option<&'static str> {
        self.default_follow_url
    }

    /// Converts the given [`Genesis`] into a [`MagnusChainSpec`].
    pub fn from_genesis(genesis: Genesis) -> Self {
        // Extract Magnus genesis info from extra_fields
        let info @ MagnusGenesisInfo { t0_time, .. } = MagnusGenesisInfo::extract_from(&genesis);

        // Create base chainspec from genesis (already has ordered Ethereum hardforks)
        let mut base_spec = ChainSpec::from_genesis(genesis);

        let magnus_forks = vec![
            (MagnusHardfork::Genesis, Some(0)),
            (MagnusHardfork::T0, t0_time),
        ]
        .into_iter()
        .filter_map(|(fork, time)| time.map(|time| (fork, ForkCondition::Timestamp(time))));
        base_spec.hardforks.extend(magnus_forks);

        Self {
            inner: base_spec.map_header(|inner| MagnusHeader {
                general_gas_limit: 0,
                timestamp_millis_part: inner.timestamp * 1000,
                shared_gas_limit: 0,
                inner,
            }),
            info,
            default_follow_url: None,
        }
    }

    /// Sets the default follow URL for this chain spec.
    pub fn with_default_follow_url(mut self, url: &'static str) -> Self {
        self.default_follow_url = Some(url);
        self
    }
}

// Required by reth's e2e-test-utils for integration tests.
// The test utilities need to convert from standard ChainSpec to custom chain specs.
impl From<ChainSpec> for MagnusChainSpec {
    fn from(spec: ChainSpec) -> Self {
        Self {
            inner: spec.map_header(|inner| MagnusHeader {
                general_gas_limit: 0,
                timestamp_millis_part: inner.timestamp * 1000,
                inner,
                shared_gas_limit: 0,
            }),
            info: MagnusGenesisInfo::default(),
            default_follow_url: None,
        }
    }
}

impl Hardforks for MagnusChainSpec {
    fn fork<H: Hardfork>(&self, fork: H) -> ForkCondition {
        self.inner.fork(fork)
    }

    fn forks_iter(&self) -> impl Iterator<Item = (&dyn Hardfork, ForkCondition)> {
        self.inner.forks_iter()
    }

    fn fork_id(&self, head: &Head) -> ForkId {
        self.inner.fork_id(head)
    }

    fn latest_fork_id(&self) -> ForkId {
        self.inner.latest_fork_id()
    }

    fn fork_filter(&self, head: Head) -> ForkFilter {
        self.inner.fork_filter(head)
    }
}

impl EthChainSpec for MagnusChainSpec {
    type Header = MagnusHeader;

    fn chain(&self) -> Chain {
        self.inner.chain()
    }

    fn base_fee_params_at_timestamp(&self, timestamp: u64) -> BaseFeeParams {
        self.inner.base_fee_params_at_timestamp(timestamp)
    }

    fn blob_params_at_timestamp(&self, timestamp: u64) -> Option<BlobParams> {
        self.inner.blob_params_at_timestamp(timestamp)
    }

    fn deposit_contract(&self) -> Option<&DepositContract> {
        self.inner.deposit_contract()
    }

    fn genesis_hash(&self) -> B256 {
        self.inner.genesis_hash()
    }

    fn prune_delete_limit(&self) -> usize {
        self.inner.prune_delete_limit()
    }

    fn display_hardforks(&self) -> Box<dyn std::fmt::Display> {
        // filter only magnus hardforks
        let magnus_forks = self.inner.hardforks.forks_iter().filter(|(fork, _)| {
            !EthereumHardfork::VARIANTS
                .iter()
                .any(|h| h.name() == (*fork).name())
        });

        Box::new(DisplayHardforks::new(magnus_forks))
    }

    fn genesis_header(&self) -> &Self::Header {
        self.inner.genesis_header()
    }

    fn genesis(&self) -> &Genesis {
        self.inner.genesis()
    }

    fn bootnodes(&self) -> Option<Vec<NodeRecord>> {
        match self.inner.chain_id() {
            42429 => Some(andantino_nodes()),
            42431 => Some(moderato_nodes()),
            _ => self.inner.bootnodes(),
        }
    }

    fn final_paris_total_difficulty(&self) -> Option<U256> {
        self.inner.get_final_paris_total_difficulty()
    }

    fn next_block_base_fee(&self, _parent: &MagnusHeader, _target_timestamp: u64) -> Option<u64> {
        Some(MAGNUS_BASE_FEE)
    }
}

impl EthereumHardforks for MagnusChainSpec {
    fn ethereum_fork_activation(&self, fork: EthereumHardfork) -> ForkCondition {
        self.inner.ethereum_fork_activation(fork)
    }
}

impl EthExecutorSpec for MagnusChainSpec {
    fn deposit_contract_address(&self) -> Option<Address> {
        self.inner.deposit_contract_address()
    }
}

impl MagnusHardforks for MagnusChainSpec {
    fn magnus_fork_activation(&self, fork: MagnusHardfork) -> ForkCondition {
        self.fork(fork)
    }
}

#[cfg(test)]
mod tests {
    use crate::hardfork::{MagnusHardfork, MagnusHardforks};
    use reth_chainspec::{ForkCondition, Hardforks};
    use reth_cli::chainspec::ChainSpecParser as _;

    #[test]
    fn can_load_testnet() {
        let _ = super::MagnusChainSpecParser::parse("testnet")
            .expect("the testnet chainspec must always be well formed");
    }

    #[test]
    fn can_load_dev() {
        let _ = super::MagnusChainSpecParser::parse("dev")
            .expect("the dev chainspec must always be well formed");
    }

    #[test]
    fn test_magnus_chainspec_has_magnus_hardforks() {
        let chainspec = super::MagnusChainSpecParser::parse("testnet")
            .expect("the testnet chainspec must always be well formed");

        // Genesis should be active at timestamp 0
        let activation = chainspec.magnus_fork_activation(MagnusHardfork::Genesis);
        assert_eq!(activation, ForkCondition::Timestamp(0));
    }

    #[test]
    fn test_magnus_chainspec_implements_magnus_hardforks_trait() {
        let chainspec = super::MagnusChainSpecParser::parse("testnet")
            .expect("the testnet chainspec must always be well formed");

        // Should be able to query Magnus hardfork activation through trait
        let activation = chainspec.magnus_fork_activation(MagnusHardfork::Genesis);
        assert_eq!(activation, ForkCondition::Timestamp(0));
    }

    #[test]
    fn test_magnus_hardforks_in_inner_hardforks() {
        let chainspec = super::MagnusChainSpecParser::parse("testnet")
            .expect("the testnet chainspec must always be well formed");

        // Magnus hardforks should be queryable from inner.hardforks via Hardforks trait
        let activation = chainspec.fork(MagnusHardfork::Genesis);
        assert_eq!(activation, ForkCondition::Timestamp(0));

        // Verify Genesis appears in forks iterator
        let has_genesis = chainspec
            .forks_iter()
            .any(|(fork, _)| fork.name() == "Genesis");
        assert!(has_genesis, "Genesis hardfork should be in inner.hardforks");
    }

    #[test]
    fn test_magnus_hardfork_at() {
        let chainspec = super::MagnusChainSpecParser::parse("testnet")
            .expect("the testnet chainspec must always be well formed");

        // Should always return Genesis
        assert_eq!(chainspec.magnus_hardfork_at(0), MagnusHardfork::Genesis);
        assert_eq!(chainspec.magnus_hardfork_at(1000), MagnusHardfork::Genesis);
        assert_eq!(
            chainspec.magnus_hardfork_at(u64::MAX),
            MagnusHardfork::Genesis
        );
    }
}
