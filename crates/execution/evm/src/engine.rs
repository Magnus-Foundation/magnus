use crate::MagnusEvmConfig;
use alloy_consensus::crypto::RecoveryError;
use alloy_evm::{RecoveredTx, ToTxEnv, block::ExecutableTxParts};
use alloy_primitives::Address;
use reth_evm::{
    ConfigureEngineEvm, ConfigureEvm, EvmEnvFor, ExecutableTxIterator, ExecutionCtxFor,
    FromRecoveredTx,
};
use reth_primitives_traits::{SealedBlock, SignedTransaction};
use std::sync::Arc;
use magnus_payload_types::MagnusExecutionData;
use magnus_primitives::{Block, MagnusTxEnvelope};
use magnus_vm::MagnusTxEnv;

impl ConfigureEngineEvm<MagnusExecutionData> for MagnusEvmConfig {
    fn evm_env_for_payload(
        &self,
        payload: &MagnusExecutionData,
    ) -> Result<EvmEnvFor<Self>, Self::Error> {
        self.evm_env(&payload.block)
    }

    fn context_for_payload<'a>(
        &self,
        payload: &'a MagnusExecutionData,
    ) -> Result<ExecutionCtxFor<'a, Self>, Self::Error> {
        let MagnusExecutionData {
            block,
            validator_set,
        } = payload;
        let mut context = self.context_for_block(block)?;

        context.validator_set = validator_set.clone();

        Ok(context)
    }

    fn tx_iterator_for_payload(
        &self,
        payload: &MagnusExecutionData,
    ) -> Result<impl ExecutableTxIterator<Self>, Self::Error> {
        let block = payload.block.clone();
        let transactions: Vec<_> = (0..payload.block.body().transactions.len())
            .map(|i| (block.clone(), i))
            .collect();

        Ok((transactions, RecoveredInBlock::new))
    }
}

/// A [`reth_evm::execute::ExecutableTxFor`] implementation that contains a pointer to the
/// block and the transaction index, allowing to prepare a [`MagnusTxEnv`] without having to
/// clone block or transaction.
#[derive(Clone)]
struct RecoveredInBlock {
    block: Arc<SealedBlock<Block>>,
    index: usize,
    sender: Address,
}

impl RecoveredInBlock {
    fn new((block, index): (Arc<SealedBlock<Block>>, usize)) -> Result<Self, RecoveryError> {
        let sender = block.body().transactions[index].try_recover()?;
        Ok(Self {
            block,
            index,
            sender,
        })
    }
}

impl RecoveredTx<MagnusTxEnvelope> for RecoveredInBlock {
    fn tx(&self) -> &MagnusTxEnvelope {
        &self.block.body().transactions[self.index]
    }

    fn signer(&self) -> &alloy_primitives::Address {
        &self.sender
    }
}

impl ToTxEnv<MagnusTxEnv> for RecoveredInBlock {
    fn to_tx_env(&self) -> MagnusTxEnv {
        MagnusTxEnv::from_recovered_tx(self.tx(), *self.signer())
    }
}

impl ExecutableTxParts<MagnusTxEnv, MagnusTxEnvelope> for RecoveredInBlock {
    type Recovered = Self;

    fn into_parts(self) -> (MagnusTxEnv, Self) {
        (self.to_tx_env(), self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_consensus::{BlockHeader, Signed, TxLegacy};
    use alloy_primitives::{B256, Bytes, Signature, TxKind, U256};
    use alloy_rlp::{Encodable, bytes::BytesMut};
    use rayon::iter::{IntoParallelIterator, ParallelIterator};
    use reth_chainspec::EthChainSpec;
    use reth_evm::ConfigureEngineEvm;
    use magnus_chainspec::{MagnusChainSpec, spec::MODERATO};
    use magnus_primitives::{
        BlockBody, SubBlockMetadata, MagnusHeader, transaction::envelope::MAGNUS_SYSTEM_TX_SIGNATURE,
    };

    fn create_legacy_tx() -> MagnusTxEnvelope {
        let tx = TxLegacy {
            chain_id: Some(1),
            nonce: 0,
            gas_price: 1,
            gas_limit: 21000,
            to: TxKind::Call(Address::repeat_byte(0x01)),
            value: U256::ZERO,
            input: Bytes::new(),
        };
        MagnusTxEnvelope::Legacy(Signed::new_unhashed(tx, Signature::test_signature()))
    }

    fn create_subblock_metadata_tx(chain_id: u64, block_number: u64) -> MagnusTxEnvelope {
        let metadata: Vec<SubBlockMetadata> = vec![];
        let mut input = BytesMut::new();
        metadata.encode(&mut input);
        input.extend_from_slice(&U256::from(block_number).to_be_bytes::<32>());

        MagnusTxEnvelope::Legacy(Signed::new_unhashed(
            TxLegacy {
                chain_id: Some(chain_id),
                nonce: 0,
                gas_price: 0,
                gas_limit: 0,
                to: TxKind::Call(Address::ZERO),
                value: U256::ZERO,
                input: input.freeze().into(),
            },
            MAGNUS_SYSTEM_TX_SIGNATURE,
        ))
    }

    fn create_test_block(transactions: Vec<MagnusTxEnvelope>) -> Arc<SealedBlock<Block>> {
        let header = MagnusHeader {
            inner: alloy_consensus::Header {
                number: 1,
                timestamp: 1000,
                gas_limit: 30_000_000,
                parent_beacon_block_root: Some(B256::ZERO),
                ..Default::default()
            },
            general_gas_limit: 10_000_000,
            timestamp_millis_part: 500,
            shared_gas_limit: 3_000_000,
        };

        let body = BlockBody {
            transactions,
            ommers: vec![],
            withdrawals: None,
        };

        let block = Block { header, body };
        Arc::new(SealedBlock::seal_slow(block))
    }

    #[test]
    fn test_tx_iterator_for_payload() {
        let chainspec = Arc::new(MagnusChainSpec::from_genesis(MODERATO.genesis().clone()));
        let evm_config = MagnusEvmConfig::new_with_default_factory(chainspec.clone());

        let tx1 = create_legacy_tx();
        let tx2 = create_legacy_tx();
        let system_tx = create_subblock_metadata_tx(chainspec.chain().id(), 1);

        let block = create_test_block(vec![tx1, tx2, system_tx]);

        let payload = MagnusExecutionData {
            block,
            validator_set: None,
        };

        let result = evm_config.tx_iterator_for_payload(&payload);
        assert!(result.is_ok());

        let tuple = result.unwrap();
        let (iter, convert) = reth_evm::ExecutableTxTuple::into_parts(tuple);
        let items: Vec<_> = iter.into_par_iter().collect();

        // Should have 3 transactions
        assert_eq!(items.len(), 3);

        // Test the recovery function works on all items
        for item in items {
            let recovered = reth_evm::ConvertTx::convert(&convert, item);
            assert!(recovered.is_ok());
        }
    }

    #[test]
    fn test_context_for_payload() {
        let chainspec = Arc::new(MagnusChainSpec::from_genesis(MODERATO.genesis().clone()));
        let evm_config = MagnusEvmConfig::new_with_default_factory(chainspec.clone());

        let system_tx = create_subblock_metadata_tx(chainspec.chain().id(), 1);
        let block = create_test_block(vec![system_tx]);
        let validator_set = Some(vec![B256::repeat_byte(0x01), B256::repeat_byte(0x02)]);

        let payload = MagnusExecutionData {
            block,
            validator_set: validator_set.clone(),
        };

        let result = evm_config.context_for_payload(&payload);
        assert!(result.is_ok());

        let context = result.unwrap();

        // Verify context fields
        assert_eq!(context.general_gas_limit, 10_000_000);
        assert_eq!(context.shared_gas_limit, 3_000_000);
        assert_eq!(context.validator_set, validator_set);
        assert!(context.subblock_fee_recipients.is_empty());
    }

    #[test]
    fn test_evm_env_for_payload() {
        let chainspec = Arc::new(MagnusChainSpec::from_genesis(MODERATO.genesis().clone()));
        let evm_config = MagnusEvmConfig::new_with_default_factory(chainspec.clone());

        let system_tx = create_subblock_metadata_tx(chainspec.chain().id(), 1);
        let block = create_test_block(vec![system_tx]);

        let payload = MagnusExecutionData {
            block: block.clone(),
            validator_set: None,
        };

        let result = evm_config.evm_env_for_payload(&payload);
        assert!(result.is_ok());

        let evm_env = result.unwrap();

        // Verify EVM environment fields
        assert_eq!(evm_env.block_env.inner.number, U256::from(block.number()));
        assert_eq!(
            evm_env.block_env.inner.timestamp,
            U256::from(block.timestamp())
        );
        assert_eq!(
            evm_env.block_env.inner.gas_limit,
            block.header().gas_limit()
        );
        assert_eq!(evm_env.block_env.timestamp_millis_part, 500);
    }
}
