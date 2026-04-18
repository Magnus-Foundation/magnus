use std::fmt::Debug;

use crate::rpc::{MagnusHeaderResponse, MagnusTransactionReceipt, MagnusTransactionRequest};
use alloy_consensus::{ReceiptWithBloom, TxType, error::UnsupportedTransactionType};

use alloy_network::{
    BuildResult, Ethereum, EthereumWallet, IntoWallet, Network, NetworkTransactionBuilder,
    NetworkWallet, TransactionBuilder, TransactionBuilderError, UnbuiltTransactionError,
};
use alloy_primitives::{Address, Bytes, ChainId, TxKind, U256};
use alloy_provider::fillers::{
    ChainIdFiller, GasFiller, JoinFill, NonceFiller, RecommendedFillers,
};
use alloy_rpc_types_eth::{AccessList, Block, Transaction};
use alloy_signer_local::PrivateKeySigner;
use magnus_primitives::{
    MagnusHeader, MagnusReceipt, MagnusTxEnvelope, MagnusTxType, transaction::MagnusTypedTransaction,
};

/// Set of recommended fillers.
///
/// `N` is a nonce filler.
pub type MagnusFillers<N> = JoinFill<N, JoinFill<GasFiller, ChainIdFiller>>;

/// The Magnus specific configuration of [`Network`] schema and consensus primitives.
#[derive(Default, Debug, Clone, Copy)]
#[non_exhaustive]
pub struct MagnusNetwork;

impl Network for MagnusNetwork {
    type TxType = MagnusTxType;
    type TxEnvelope = MagnusTxEnvelope;
    type UnsignedTx = MagnusTypedTransaction;
    type ReceiptEnvelope = ReceiptWithBloom<MagnusReceipt>;
    type Header = MagnusHeader;
    type TransactionRequest = MagnusTransactionRequest;
    type TransactionResponse = Transaction<MagnusTxEnvelope>;
    type ReceiptResponse = MagnusTransactionReceipt;
    type HeaderResponse = MagnusHeaderResponse;
    type BlockResponse = Block<Transaction<MagnusTxEnvelope>, Self::HeaderResponse>;
}

impl TransactionBuilder for MagnusTransactionRequest {
    fn chain_id(&self) -> Option<ChainId> {
        TransactionBuilder::chain_id(&self.inner)
    }

    fn set_chain_id(&mut self, chain_id: ChainId) {
        TransactionBuilder::set_chain_id(&mut self.inner, chain_id)
    }

    fn nonce(&self) -> Option<u64> {
        TransactionBuilder::nonce(&self.inner)
    }

    fn set_nonce(&mut self, nonce: u64) {
        TransactionBuilder::set_nonce(&mut self.inner, nonce)
    }

    fn take_nonce(&mut self) -> Option<u64> {
        TransactionBuilder::take_nonce(&mut self.inner)
    }

    fn input(&self) -> Option<&Bytes> {
        TransactionBuilder::input(&self.inner)
    }

    fn set_input<T: Into<Bytes>>(&mut self, input: T) {
        TransactionBuilder::set_input(&mut self.inner, input)
    }

    fn from(&self) -> Option<Address> {
        TransactionBuilder::from(&self.inner)
    }

    fn set_from(&mut self, from: Address) {
        TransactionBuilder::set_from(&mut self.inner, from)
    }

    fn kind(&self) -> Option<TxKind> {
        TransactionBuilder::kind(&self.inner)
    }

    fn clear_kind(&mut self) {
        TransactionBuilder::clear_kind(&mut self.inner)
    }

    fn set_kind(&mut self, kind: TxKind) {
        TransactionBuilder::set_kind(&mut self.inner, kind)
    }

    fn value(&self) -> Option<U256> {
        TransactionBuilder::value(&self.inner)
    }

    fn set_value(&mut self, value: U256) {
        TransactionBuilder::set_value(&mut self.inner, value)
    }

    fn gas_price(&self) -> Option<u128> {
        TransactionBuilder::gas_price(&self.inner)
    }

    fn set_gas_price(&mut self, gas_price: u128) {
        TransactionBuilder::set_gas_price(&mut self.inner, gas_price)
    }

    fn max_fee_per_gas(&self) -> Option<u128> {
        TransactionBuilder::max_fee_per_gas(&self.inner)
    }

    fn set_max_fee_per_gas(&mut self, max_fee_per_gas: u128) {
        TransactionBuilder::set_max_fee_per_gas(&mut self.inner, max_fee_per_gas)
    }

    fn max_priority_fee_per_gas(&self) -> Option<u128> {
        TransactionBuilder::max_priority_fee_per_gas(&self.inner)
    }

    fn set_max_priority_fee_per_gas(&mut self, max_priority_fee_per_gas: u128) {
        TransactionBuilder::set_max_priority_fee_per_gas(&mut self.inner, max_priority_fee_per_gas)
    }

    fn gas_limit(&self) -> Option<u64> {
        TransactionBuilder::gas_limit(&self.inner)
    }

    fn set_gas_limit(&mut self, gas_limit: u64) {
        TransactionBuilder::set_gas_limit(&mut self.inner, gas_limit)
    }

    fn access_list(&self) -> Option<&AccessList> {
        TransactionBuilder::access_list(&self.inner)
    }

    fn set_access_list(&mut self, access_list: AccessList) {
        TransactionBuilder::set_access_list(&mut self.inner, access_list)
    }
}

impl NetworkTransactionBuilder<MagnusNetwork> for MagnusTransactionRequest {
    fn complete_type(&self, ty: MagnusTxType) -> Result<(), Vec<&'static str>> {
        match ty {
            MagnusTxType::AA => self.complete_aa(),
            MagnusTxType::Legacy
            | MagnusTxType::Eip2930
            | MagnusTxType::Eip1559
            | MagnusTxType::Eip7702 => NetworkTransactionBuilder::<Ethereum>::complete_type(
                &self.inner,
                ty.try_into().expect("magnus tx types checked"),
            ),
        }
    }

    fn can_submit(&self) -> bool {
        NetworkTransactionBuilder::<Ethereum>::can_submit(&self.inner)
    }

    fn can_build(&self) -> bool {
        NetworkTransactionBuilder::<Ethereum>::can_build(&self.inner) || self.can_build_aa()
    }

    fn output_tx_type(&self) -> MagnusTxType {
        if !self.calls.is_empty()
            || self.nonce_key.is_some()
            || self.fee_token.is_some()
            || !self.magnus_authorization_list.is_empty()
            || self.key_authorization.is_some()
            || self.key_id.is_some()
            || self.key_type.is_some()
            || self.key_data.is_some()
            || self.valid_before.is_some()
            || self.valid_after.is_some()
            || self.fee_payer_signature.is_some()
        {
            MagnusTxType::AA
        } else {
            match NetworkTransactionBuilder::<Ethereum>::output_tx_type(&self.inner) {
                TxType::Legacy => MagnusTxType::Legacy,
                TxType::Eip2930 => MagnusTxType::Eip2930,
                TxType::Eip1559 => MagnusTxType::Eip1559,
                // EIP-4844 transactions are not supported on Magnus
                TxType::Eip4844 => MagnusTxType::Legacy,
                TxType::Eip7702 => MagnusTxType::Eip7702,
            }
        }
    }

    fn output_tx_type_checked(&self) -> Option<MagnusTxType> {
        match self.output_tx_type() {
            MagnusTxType::AA => Some(MagnusTxType::AA).filter(|_| self.can_build_aa()),
            MagnusTxType::Legacy
            | MagnusTxType::Eip2930
            | MagnusTxType::Eip1559
            | MagnusTxType::Eip7702 => {
                NetworkTransactionBuilder::<Ethereum>::output_tx_type_checked(&self.inner)?
                    .try_into()
                    .ok()
            }
        }
    }

    fn prep_for_submission(&mut self) {
        self.inner.transaction_type = Some(self.output_tx_type() as u8);
        self.inner.trim_conflicting_keys();
        self.inner.populate_blob_hashes();
    }

    fn build_unsigned(self) -> BuildResult<MagnusTypedTransaction, MagnusNetwork> {
        match self.output_tx_type() {
            MagnusTxType::AA => match self.complete_aa() {
                Ok(..) => Ok(self.build_aa().expect("checked by above condition").into()),
                Err(missing) => Err(TransactionBuilderError::InvalidTransactionRequest(
                    MagnusTxType::AA,
                    missing,
                )
                .into_unbuilt(self)),
            },
            _ => {
                if let Err((tx_type, missing)) = self.inner.missing_keys() {
                    return Err(match tx_type.try_into() {
                        Ok(tx_type) => {
                            TransactionBuilderError::InvalidTransactionRequest(tx_type, missing)
                        }
                        Err(err) => TransactionBuilderError::from(err),
                    }
                    .into_unbuilt(self));
                }

                if let Some(TxType::Eip4844) = self.inner.buildable_type() {
                    return Err(UnbuiltTransactionError {
                        request: self,
                        error: TransactionBuilderError::Custom(Box::new(
                            UnsupportedTransactionType::new(TxType::Eip4844),
                        )),
                    });
                }

                let inner = self
                    .inner
                    .build_typed_tx()
                    .expect("checked by missing_keys");

                Ok(inner.try_into().expect("checked by above condition"))
            }
        }
    }

    async fn build<W: NetworkWallet<MagnusNetwork>>(
        self,
        wallet: &W,
    ) -> Result<MagnusTxEnvelope, TransactionBuilderError<MagnusNetwork>> {
        Ok(wallet.sign_request(self).await?)
    }
}

impl MagnusTransactionRequest {
    fn can_build_aa(&self) -> bool {
        (!self.calls.is_empty() || self.inner.to.is_some())
            && self.inner.nonce.is_some()
            && self.inner.gas.is_some()
            && self.inner.max_fee_per_gas.is_some()
            && self.inner.max_priority_fee_per_gas.is_some()
    }

    fn complete_aa(&self) -> Result<(), Vec<&'static str>> {
        let mut fields = Vec::new();

        if self.calls.is_empty() && self.inner.to.is_none() {
            fields.push("calls or to");
        }
        if self.inner.nonce.is_none() {
            fields.push("nonce");
        }
        if self.inner.gas.is_none() {
            fields.push("gas");
        }
        if self.inner.max_fee_per_gas.is_none() {
            fields.push("max_fee_per_gas");
        }
        if self.inner.max_priority_fee_per_gas.is_none() {
            fields.push("max_priority_fee_per_gas");
        }

        if fields.is_empty() {
            Ok(())
        } else {
            Err(fields)
        }
    }
}

impl RecommendedFillers for MagnusNetwork {
    type RecommendedFillers = MagnusFillers<NonceFiller>;

    fn recommended_fillers() -> Self::RecommendedFillers {
        Default::default()
    }
}

impl IntoWallet<MagnusNetwork> for PrivateKeySigner {
    type NetworkWallet = EthereumWallet;

    fn into_wallet(self) -> Self::NetworkWallet {
        self.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_consensus::{TxEip1559, TxEip2930, TxEip7702, TxLegacy};
    use alloy_eips::eip7702::SignedAuthorization;
    use alloy_primitives::{B256, Signature};
    use alloy_rpc_types_eth::{AccessListItem, Authorization, TransactionRequest};
    use magnus_primitives::{
        SignatureType, MagnusSignature,
        transaction::{KeyAuthorization, PrimitiveSignature, MagnusSignedAuthorization},
    };

    #[test_case::test_case(
        MagnusTransactionRequest {
            inner: TransactionRequest {
                to: Some(TxKind::Call(Address::repeat_byte(0xDE))),
                gas_price: Some(1234),
                nonce: Some(57),
                gas: Some(123456),
                ..Default::default()
            },
            ..Default::default()
        },
        MagnusTypedTransaction::Legacy(TxLegacy {
            to: TxKind::Call(Address::repeat_byte(0xDE)),
            gas_price: 1234,
            nonce: 57,
            gas_limit: 123456,
            ..Default::default()
        });
        "Legacy"
    )]
    #[test_case::test_case(
        MagnusTransactionRequest {
            inner: TransactionRequest {
                to: Some(TxKind::Call(Address::repeat_byte(0xDE))),
                max_fee_per_gas: Some(1234),
                max_priority_fee_per_gas: Some(987),
                nonce: Some(57),
                gas: Some(123456),
                ..Default::default()
            },
            ..Default::default()
        },
        MagnusTypedTransaction::Eip1559(TxEip1559 {
            to: TxKind::Call(Address::repeat_byte(0xDE)),
            max_fee_per_gas: 1234,
            max_priority_fee_per_gas: 987,
            nonce: 57,
            gas_limit: 123456,
            chain_id: 1,
            ..Default::default()
        });
        "EIP-1559"
    )]
    #[test_case::test_case(
        MagnusTransactionRequest {
            inner: TransactionRequest {
                to: Some(TxKind::Call(Address::repeat_byte(0xDE))),
                gas_price: Some(1234),
                nonce: Some(57),
                gas: Some(123456),
                access_list: Some(AccessList(vec![AccessListItem {
                    address: Address::from([3u8; 20]),
                    storage_keys: vec![B256::from([4u8; 32])],
                }])),
                ..Default::default()
            },
            ..Default::default()
        },
        MagnusTypedTransaction::Eip2930(TxEip2930 {
            to: TxKind::Call(Address::repeat_byte(0xDE)),
            gas_price: 1234,
            nonce: 57,
            gas_limit: 123456,
            chain_id: 1,
            access_list: AccessList(vec![AccessListItem {
                address: Address::from([3u8; 20]),
                storage_keys: vec![B256::from([4u8; 32])],
            }]),
            ..Default::default()
        });
        "EIP-2930"
    )]
    #[test_case::test_case(
        MagnusTransactionRequest {
            inner: TransactionRequest {
                to: Some(TxKind::Call(Address::repeat_byte(0xDE))),
                max_fee_per_gas: Some(1234),
                max_priority_fee_per_gas: Some(987),
                nonce: Some(57),
                gas: Some(123456),
                authorization_list: Some(vec![SignedAuthorization::new_unchecked(
                    Authorization {
                        chain_id: U256::from(1337),
                        address: Address::ZERO,
                        nonce: 0
                    },
                    0,
                    U256::ZERO,
                    U256::ZERO,
                )]),
                ..Default::default()
            },
            ..Default::default()
        },
        MagnusTypedTransaction::Eip7702(TxEip7702 {
            to: Address::repeat_byte(0xDE),
            max_fee_per_gas: 1234,
            max_priority_fee_per_gas: 987,
            nonce: 57,
            gas_limit: 123456,
            chain_id: 1,
            authorization_list: vec![SignedAuthorization::new_unchecked(
                Authorization {
                    chain_id: U256::from(1337),
                    address: Address::ZERO,
                    nonce: 0
                },
                0,
                U256::ZERO,
                U256::ZERO,
            )],
            ..Default::default()
        });
        "EIP-7702"
    )]
    fn test_transaction_builds_successfully(
        request: MagnusTransactionRequest,
        expected_transaction: MagnusTypedTransaction,
    ) {
        let actual_transaction = request
            .build_unsigned()
            .expect("required fields should be filled out");

        assert_eq!(actual_transaction, expected_transaction);
    }

    #[test_case::test_case(
        MagnusTransactionRequest {
            inner: TransactionRequest {
                to: Some(TxKind::Call(Address::repeat_byte(0xDE))),
                max_priority_fee_per_gas: Some(987),
                nonce: Some(57),
                gas: Some(123456),
                ..Default::default()
            },
            ..Default::default()
        },
        "Failed to build transaction: EIP-1559 transaction can't be built due to missing keys: [\"max_fee_per_gas\"]";
        "EIP-1559 missing max fee"
    )]
    fn test_transaction_fails_to_build(request: MagnusTransactionRequest, expected_error: &str) {
        let actual_error = request
            .build_unsigned()
            .expect_err("some required fields should be missing")
            .to_string();

        assert_eq!(actual_error, expected_error);
    }

    #[test]
    fn output_tx_type_empty_request_is_not_aa() {
        let req = MagnusTransactionRequest::default();
        assert_ne!(req.output_tx_type(), MagnusTxType::AA);
    }

    #[test]
    fn output_tx_type_tempo_authorization_list_is_aa() {
        let req = MagnusTransactionRequest {
            magnus_authorization_list: vec![MagnusSignedAuthorization::new_unchecked(
                Authorization {
                    chain_id: U256::ZERO,
                    address: Address::ZERO,
                    nonce: 0,
                },
                MagnusSignature::Primitive(PrimitiveSignature::Secp256k1(Signature::new(
                    U256::ZERO,
                    U256::ZERO,
                    false,
                ))),
            )],
            ..Default::default()
        };
        assert_eq!(req.output_tx_type(), MagnusTxType::AA);
    }

    #[test]
    fn output_tx_type_key_authorization_is_aa() {
        let req = MagnusTransactionRequest {
            key_authorization: Some(
                KeyAuthorization::unrestricted(0, SignatureType::Secp256k1, Address::ZERO)
                    .into_signed(PrimitiveSignature::Secp256k1(Signature::new(
                        U256::ZERO,
                        U256::ZERO,
                        false,
                    ))),
            ),
            ..Default::default()
        };
        assert_eq!(req.output_tx_type(), MagnusTxType::AA);
    }

    #[test]
    fn output_tx_type_key_id_is_aa() {
        let req = MagnusTransactionRequest {
            key_id: Some(Address::ZERO),
            ..Default::default()
        };
        assert_eq!(req.output_tx_type(), MagnusTxType::AA);
    }

    #[test]
    fn output_tx_type_fee_payer_signature_is_aa() {
        let req = MagnusTransactionRequest {
            fee_payer_signature: Some(Signature::new(U256::ZERO, U256::ZERO, false)),
            ..Default::default()
        };
        assert_eq!(req.output_tx_type(), MagnusTxType::AA);
    }

    #[test]
    fn output_tx_type_validity_window_is_aa() {
        let req = MagnusTransactionRequest {
            valid_before: Some(core::num::NonZeroU64::new(1000).unwrap()),
            ..Default::default()
        };
        assert_eq!(req.output_tx_type(), MagnusTxType::AA);

        let req = MagnusTransactionRequest {
            valid_after: Some(core::num::NonZeroU64::new(500).unwrap()),
            ..Default::default()
        };
        assert_eq!(req.output_tx_type(), MagnusTxType::AA);
    }
}
