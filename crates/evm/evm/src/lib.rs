//! Magnus EVM implementation.

#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod assemble;
use alloy_consensus::{BlockHeader as _, Transaction};
use alloy_primitives::Address;
use alloy_rlp::Decodable;
pub use assemble::MagnusBlockAssembler;
mod block;
pub use block::MagnusReceiptBuilder;
mod context;
pub use context::{MagnusBlockExecutionCtx, MagnusNextBlockEnvAttributes};
#[cfg(feature = "engine")]
mod engine;
#[cfg(feature = "engine")]
use rayon as _;
mod error;
pub use error::MagnusEvmError;
pub mod evm;
use std::{borrow::Cow, sync::Arc};

use alloy_evm::{
    self, EvmEnv,
    block::{BlockExecutorFactory, BlockExecutorFor},
    eth::{EthBlockExecutionCtx, NextEvmEnvAttributes},
    revm::Inspector,
};
pub use evm::MagnusEvmFactory;
use reth_chainspec::EthChainSpec;
use reth_evm::{self, ConfigureEvm, EvmEnvFor, block::StateDB};
use reth_primitives_traits::{SealedBlock, SealedHeader};
use magnus_primitives::{
    Block, SubBlockMetadata, MagnusHeader, MagnusPrimitives, MagnusReceipt, MagnusTxEnvelope,
    subblock::PartialValidatorKey,
};

use crate::{block::MagnusBlockExecutor, evm::MagnusEvm};
use reth_evm_ethereum::EthEvmConfig;
use magnus_chainspec::{MagnusChainSpec, hardfork::MagnusHardforks};
use magnus_revm::{evm::MagnusContext, gas_params::magnus_gas_params};

pub use magnus_revm::{MagnusBlockEnv, MagnusHaltReason, MagnusStateAccess};

#[cfg(test)]
mod test_utils;

/// Magnus-related EVM configuration.
#[derive(Debug, Clone)]
pub struct MagnusEvmConfig {
    /// Inner evm config
    pub inner: EthEvmConfig<MagnusChainSpec, MagnusEvmFactory>,

    /// Block assembler
    pub block_assembler: MagnusBlockAssembler,
}

impl MagnusEvmConfig {
    /// Create a new [`MagnusEvmConfig`] with the given chain spec and EVM factory.
    pub fn new(chain_spec: Arc<MagnusChainSpec>) -> Self {
        let inner =
            EthEvmConfig::new_with_evm_factory(chain_spec.clone(), MagnusEvmFactory::default());
        Self {
            inner,
            block_assembler: MagnusBlockAssembler::new(chain_spec),
        }
    }

    /// Returns the chain spec
    pub const fn chain_spec(&self) -> &Arc<MagnusChainSpec> {
        self.inner.chain_spec()
    }

    /// Returns the inner EVM config
    pub const fn inner(&self) -> &EthEvmConfig<MagnusChainSpec, MagnusEvmFactory> {
        &self.inner
    }

    /// Returns the moderato EVM config.
    pub fn moderato() -> Self {
        Self::new(Arc::new(MagnusChainSpec::moderato()))
    }

    /// Returns the mainnet EVM config.
    pub fn mainnet() -> Self {
        Self::new(Arc::new(MagnusChainSpec::mainnet()))
    }
}

impl BlockExecutorFactory for MagnusEvmConfig {
    type EvmFactory = MagnusEvmFactory;
    type ExecutionCtx<'a> = MagnusBlockExecutionCtx<'a>;
    type Transaction = MagnusTxEnvelope;
    type Receipt = MagnusReceipt;

    fn evm_factory(&self) -> &Self::EvmFactory {
        self.inner.executor_factory.evm_factory()
    }

    fn create_executor<'a, DB, I>(
        &'a self,
        evm: MagnusEvm<DB, I>,
        ctx: Self::ExecutionCtx<'a>,
    ) -> impl BlockExecutorFor<'a, Self, DB, I>
    where
        DB: StateDB + 'a,
        I: Inspector<MagnusContext<DB>> + 'a,
    {
        MagnusBlockExecutor::new(evm, ctx, self.chain_spec())
    }
}

impl ConfigureEvm for MagnusEvmConfig {
    type Primitives = MagnusPrimitives;
    type Error = MagnusEvmError;
    type NextBlockEnvCtx = MagnusNextBlockEnvAttributes;
    type BlockExecutorFactory = Self;
    type BlockAssembler = MagnusBlockAssembler;

    fn block_executor_factory(&self) -> &Self::BlockExecutorFactory {
        self
    }

    fn block_assembler(&self) -> &Self::BlockAssembler {
        &self.block_assembler
    }

    fn evm_env(&self, header: &MagnusHeader) -> Result<EvmEnvFor<Self>, Self::Error> {
        let EvmEnv { cfg_env, block_env } = EvmEnv::for_eth_block(
            header,
            self.chain_spec(),
            self.chain_spec().chain().id(),
            self.chain_spec()
                .blob_params_at_timestamp(header.timestamp()),
        );

        let spec = self.chain_spec().magnus_hardfork_at(header.timestamp());

        // Apply MIP-1000 gas params for T1 hardfork.
        let mut cfg_env = cfg_env.with_spec_and_gas_params(spec, magnus_gas_params(spec));
        cfg_env.tx_gas_limit_cap = spec.tx_gas_limit_cap();

        Ok(EvmEnv {
            cfg_env,
            block_env: MagnusBlockEnv {
                inner: block_env,
                timestamp_millis_part: header.timestamp_millis_part,
            },
        })
    }

    fn next_evm_env(
        &self,
        parent: &MagnusHeader,
        attributes: &Self::NextBlockEnvCtx,
    ) -> Result<EvmEnvFor<Self>, Self::Error> {
        let EvmEnv { cfg_env, block_env } = EvmEnv::for_eth_next_block(
            parent,
            NextEvmEnvAttributes {
                timestamp: attributes.timestamp,
                suggested_fee_recipient: attributes.suggested_fee_recipient,
                prev_randao: attributes.prev_randao,
                gas_limit: attributes.gas_limit,
                slot_number: attributes.slot_number,
            },
            self.chain_spec()
                .next_block_base_fee(parent, attributes.timestamp)
                .unwrap_or_default(),
            self.chain_spec(),
            self.chain_spec().chain().id(),
            self.chain_spec()
                .blob_params_at_timestamp(attributes.timestamp),
        );

        let spec = self.chain_spec().magnus_hardfork_at(attributes.timestamp);

        // Apply MIP-1000 gas params for T1 hardfork.
        let mut cfg_env = cfg_env.with_spec_and_gas_params(spec, magnus_gas_params(spec));
        cfg_env.tx_gas_limit_cap = spec.tx_gas_limit_cap();

        Ok(EvmEnv {
            cfg_env,
            block_env: MagnusBlockEnv {
                inner: block_env,
                timestamp_millis_part: attributes.timestamp_millis_part,
            },
        })
    }

    fn context_for_block<'a>(
        &self,
        block: &'a SealedBlock<Block>,
    ) -> Result<MagnusBlockExecutionCtx<'a>, Self::Error> {
        // Decode validator -> fee_recipient mapping from the subblock metadata system transaction.
        let subblock_fee_recipients = block
            .body()
            .transactions
            .iter()
            .rev()
            .filter(|tx| (*tx).to() == Some(Address::ZERO))
            .find_map(|tx| Vec::<SubBlockMetadata>::decode(&mut tx.input().as_ref()).ok())
            .ok_or(MagnusEvmError::NoSubblockMetadataFound)?
            .into_iter()
            .map(|metadata| {
                (
                    PartialValidatorKey::from_slice(&metadata.validator[..15]),
                    metadata.fee_recipient,
                )
            })
            .collect();

        Ok(MagnusBlockExecutionCtx {
            inner: EthBlockExecutionCtx {
                parent_hash: block.header().parent_hash(),
                parent_beacon_block_root: block.header().parent_beacon_block_root(),
                // no ommers in magnus
                ommers: &[],
                withdrawals: block
                    .body()
                    .withdrawals
                    .as_ref()
                    .map(|w| Cow::Borrowed(w.as_slice())),
                extra_data: block.extra_data().clone(),
                tx_count_hint: Some(block.body().transactions.len()),
            },
            general_gas_limit: block.header().general_gas_limit,
            shared_gas_limit: block.header().gas_limit()
                / magnus_consensus::MAGNUS_SHARED_GAS_DIVISOR,
            // Not available when we only have a block body.
            validator_set: None,
            consensus_context: block.header().consensus_context,
            subblock_fee_recipients,
        })
    }

    fn context_for_next_block(
        &self,
        parent: &SealedHeader<MagnusHeader>,
        attributes: Self::NextBlockEnvCtx,
    ) -> Result<MagnusBlockExecutionCtx<'_>, Self::Error> {
        Ok(MagnusBlockExecutionCtx {
            inner: EthBlockExecutionCtx {
                parent_hash: parent.hash(),
                parent_beacon_block_root: attributes.parent_beacon_block_root,
                ommers: &[],
                withdrawals: attributes
                    .inner
                    .withdrawals
                    .map(|w| Cow::Owned(w.into_inner())),
                extra_data: attributes.inner.extra_data,
                tx_count_hint: None,
            },
            general_gas_limit: attributes.general_gas_limit,
            shared_gas_limit: attributes.inner.gas_limit
                / magnus_consensus::MAGNUS_SHARED_GAS_DIVISOR,
            // Fine to not validate during block building.
            validator_set: None,
            consensus_context: attributes.consensus_context,
            subblock_fee_recipients: attributes.subblock_fee_recipients,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_chainspec;
    use alloy_consensus::{BlockHeader, Signed, TxLegacy};
    use alloy_primitives::{B256, Bytes, Signature, TxKind, U256};
    use alloy_rlp::{Encodable, bytes::BytesMut};
    use reth_evm::{ConfigureEvm, NextBlockEnvAttributes};
    use std::collections::HashMap;
    use magnus_chainspec::hardfork::MagnusHardfork;
    use magnus_primitives::{
        BlockBody, SubBlockMetadata, subblock::SubBlockVersion,
        transaction::envelope::MAGNUS_SYSTEM_TX_SIGNATURE,
    };

    #[test]
    fn test_evm_config_can_query_magnus_hardforks() {
        let evm_config = MagnusEvmConfig::new(test_chainspec());
        let activation = evm_config
            .chain_spec()
            .magnus_fork_activation(MagnusHardfork::Genesis);
        assert_eq!(activation, reth_chainspec::ForkCondition::Timestamp(0));
    }

    #[test]
    fn test_evm_env() {
        let evm_config = MagnusEvmConfig::new(test_chainspec());

        let header = MagnusHeader {
            inner: alloy_consensus::Header {
                number: 100,
                timestamp: 1000,
                gas_limit: 30_000_000,
                base_fee_per_gas: Some(1000),
                beneficiary: alloy_primitives::Address::repeat_byte(0x01),
                ..Default::default()
            },
            general_gas_limit: 10_000_000,
            timestamp_millis_part: 500,
            shared_gas_limit: 3_000_000,
            ..Default::default()
        };

        let result = evm_config.evm_env(&header);
        assert!(result.is_ok());

        let evm_env = result.unwrap();

        // Verify block env fields
        assert_eq!(evm_env.block_env.inner.number, U256::from(header.number()));
        assert_eq!(
            evm_env.block_env.inner.timestamp,
            U256::from(header.timestamp())
        );
        assert_eq!(evm_env.block_env.inner.gas_limit, header.gas_limit());
        assert_eq!(evm_env.block_env.inner.beneficiary, header.beneficiary());

        // Verify Magnus-specific field
        assert_eq!(evm_env.block_env.timestamp_millis_part, 500);
    }

    /// Test that evm_env sets 30M gas limit cap for T1 hardfork as per [MIP-1000].
    ///
    /// [MIP-1000]: <https://docs.magnus.xyz/protocol/mips/mip-1000>
    #[test]
    fn test_evm_env_t1_gas_cap() {
        use magnus_chainspec::spec::DEV;

        // DEV chainspec has T1 activated at timestamp 0
        let chainspec = DEV.clone();
        let evm_config = MagnusEvmConfig::new(chainspec.clone());

        let header = MagnusHeader {
            inner: alloy_consensus::Header {
                number: 100,
                timestamp: 1000, // After T1 activation
                gas_limit: 30_000_000,
                base_fee_per_gas: Some(1000),
                ..Default::default()
            },
            general_gas_limit: 10_000_000,
            timestamp_millis_part: 0,
            shared_gas_limit: 3_000_000,
            ..Default::default()
        };

        // Verify we're in T1
        assert!(chainspec.magnus_hardfork_at(header.timestamp()).is_t1());

        let evm_env = evm_config.evm_env(&header).unwrap();

        // Verify MIP-1000 gas limit cap is set
        assert_eq!(
            evm_env.cfg_env.tx_gas_limit_cap,
            Some(magnus_chainspec::spec::MAGNUS_T1_TX_GAS_LIMIT_CAP),
            "MIP-1000 requires 30M gas limit cap for T1 hardfork"
        );
    }

    #[test]
    fn test_next_evm_env() {
        let evm_config = MagnusEvmConfig::new(test_chainspec());

        let parent = MagnusHeader {
            inner: alloy_consensus::Header {
                number: 99,
                timestamp: 900,
                gas_limit: 30_000_000,
                base_fee_per_gas: Some(1000),
                ..Default::default()
            },
            general_gas_limit: 10_000_000,
            timestamp_millis_part: 0,
            shared_gas_limit: 3_000_000,
            ..Default::default()
        };

        let attributes = MagnusNextBlockEnvAttributes {
            inner: NextBlockEnvAttributes {
                timestamp: 1000,
                suggested_fee_recipient: alloy_primitives::Address::repeat_byte(0x02),
                prev_randao: B256::repeat_byte(0x03),
                gas_limit: 30_000_000,
                parent_beacon_block_root: Some(B256::ZERO),
                withdrawals: None,
                extra_data: Default::default(),
                slot_number: None,
            },
            general_gas_limit: 10_000_000,
            shared_gas_limit: 3_000_000,
            timestamp_millis_part: 750,
            consensus_context: None,
            subblock_fee_recipients: HashMap::new(),
        };

        let result = evm_config.next_evm_env(&parent, &attributes);
        assert!(result.is_ok());

        let evm_env = result.unwrap();

        // Verify block env uses attributes
        // parent + 1
        assert_eq!(evm_env.block_env.inner.number, U256::from(100));
        assert_eq!(evm_env.block_env.inner.timestamp, U256::from(1000));
        assert_eq!(
            evm_env.block_env.inner.beneficiary,
            Address::repeat_byte(0x02)
        );
        assert_eq!(evm_env.block_env.inner.gas_limit, 30_000_000);

        // Verify Magnus-specific field
        assert_eq!(evm_env.block_env.timestamp_millis_part, 750);
    }

    #[test]
    fn test_context_for_block() {
        let chainspec = test_chainspec();
        let evm_config = MagnusEvmConfig::new(chainspec.clone());

        // Create subblock metadata
        let validator_key = B256::repeat_byte(0x01);
        let fee_recipient = alloy_primitives::Address::repeat_byte(0x02);
        let metadata = vec![SubBlockMetadata {
            version: SubBlockVersion::V1,
            validator: validator_key,
            fee_recipient,
            signature: Bytes::from_static(&[0; 64]),
        }];

        // Create system tx with metadata
        let block_number = 1u64;
        let mut input = BytesMut::new();
        metadata.encode(&mut input);
        input.extend_from_slice(&U256::from(block_number).to_be_bytes::<32>());

        let system_tx = MagnusTxEnvelope::Legacy(Signed::new_unhashed(
            TxLegacy {
                chain_id: Some(reth_chainspec::EthChainSpec::chain(&*chainspec).id()),
                nonce: 0,
                gas_price: 0,
                gas_limit: 0,
                to: TxKind::Call(alloy_primitives::Address::ZERO),
                value: U256::ZERO,
                input: input.freeze().into(),
            },
            MAGNUS_SYSTEM_TX_SIGNATURE,
        ));

        let header = MagnusHeader {
            inner: alloy_consensus::Header {
                number: block_number,
                timestamp: 1000,
                gas_limit: 30_000_000,
                parent_beacon_block_root: Some(B256::ZERO),
                ..Default::default()
            },
            general_gas_limit: 10_000_000,
            timestamp_millis_part: 500,
            shared_gas_limit: 3_000_000,
            ..Default::default()
        };

        let body = BlockBody {
            transactions: vec![system_tx],
            ommers: vec![],
            withdrawals: None,
        };

        let block = Block { header, body };
        let sealed_block = SealedBlock::seal_slow(block);

        let result = evm_config.context_for_block(&sealed_block);
        assert!(result.is_ok());

        let context = result.unwrap();

        // Verify context fields
        assert_eq!(context.general_gas_limit, 10_000_000);
        assert_eq!(context.shared_gas_limit, 3_000_000);
        assert!(context.validator_set.is_none());

        // Verify subblock_fee_recipients was extracted from metadata
        let partial_key = PartialValidatorKey::from_slice(&validator_key[..15]);
        assert_eq!(
            context.subblock_fee_recipients.get(&partial_key),
            Some(&fee_recipient)
        );
    }

    #[test]
    fn test_context_for_block_no_subblock_metadata() {
        let evm_config = MagnusEvmConfig::new(test_chainspec());

        // Create a block without subblock metadata system tx
        let regular_tx = MagnusTxEnvelope::Legacy(Signed::new_unhashed(
            TxLegacy {
                chain_id: Some(1),
                nonce: 0,
                gas_price: 1,
                gas_limit: 21000,
                to: TxKind::Call(alloy_primitives::Address::repeat_byte(0x01)),
                value: U256::ZERO,
                input: Bytes::new(),
            },
            Signature::test_signature(),
        ));

        let header = MagnusHeader {
            inner: alloy_consensus::Header {
                number: 1,
                timestamp: 1000,
                gas_limit: 30_000_000,
                ..Default::default()
            },
            general_gas_limit: 10_000_000,
            timestamp_millis_part: 500,
            shared_gas_limit: 3_000_000,
            ..Default::default()
        };

        let body = BlockBody {
            transactions: vec![regular_tx],
            ommers: vec![],
            withdrawals: None,
        };

        let block = Block { header, body };
        let sealed_block = SealedBlock::seal_slow(block);

        let result = evm_config.context_for_block(&sealed_block);

        // Should fail because no subblock metadata tx was found
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            MagnusEvmError::NoSubblockMetadataFound
        ));
    }

    #[test]
    fn test_context_for_next_block() {
        let evm_config = MagnusEvmConfig::new(test_chainspec());

        let parent_header = MagnusHeader {
            inner: alloy_consensus::Header {
                number: 99,
                timestamp: 900,
                gas_limit: 30_000_000,
                ..Default::default()
            },
            general_gas_limit: 10_000_000,
            timestamp_millis_part: 0,
            shared_gas_limit: 3_000_000,
            ..Default::default()
        };
        let parent = SealedHeader::seal_slow(parent_header);

        let fee_recipient = Address::repeat_byte(0x02);
        let mut subblock_fee_recipients = HashMap::new();
        let partial_key = PartialValidatorKey::from_slice(&[0x01; 15]);
        subblock_fee_recipients.insert(partial_key, fee_recipient);

        let attributes = MagnusNextBlockEnvAttributes {
            inner: NextBlockEnvAttributes {
                timestamp: 1000,
                suggested_fee_recipient: alloy_primitives::Address::repeat_byte(0x03),
                prev_randao: B256::repeat_byte(0x04),
                gas_limit: 30_000_000,
                parent_beacon_block_root: Some(B256::repeat_byte(0x05)),
                withdrawals: None,
                extra_data: Default::default(),
                slot_number: None,
            },
            general_gas_limit: 12_000_000,
            shared_gas_limit: 4_000_000,
            timestamp_millis_part: 999,
            consensus_context: None,
            subblock_fee_recipients: subblock_fee_recipients.clone(),
        };

        let result = evm_config.context_for_next_block(&parent, attributes);
        assert!(result.is_ok());

        let context = result.unwrap();

        // Verify context fields from attributes
        assert_eq!(context.general_gas_limit, 12_000_000);
        assert_eq!(context.shared_gas_limit, 3_000_000);
        assert!(context.validator_set.is_none());
        assert_eq!(context.inner.parent_hash, parent.hash());
        assert_eq!(
            context.inner.parent_beacon_block_root,
            Some(B256::repeat_byte(0x05))
        );

        // Verify subblock_fee_recipients passed through
        assert_eq!(
            context.subblock_fee_recipients.get(&partial_key),
            Some(&fee_recipient)
        );
    }
}
