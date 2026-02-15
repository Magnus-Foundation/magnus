use alloy_consensus::{Signed, TxEip1559, TxEip2930, TxEip7702, TxLegacy, error::ValueError};
use alloy_contract::{CallBuilder, CallDecoder};
use alloy_eips::Typed2718;
use alloy_primitives::{Address, Bytes, U256};
use alloy_provider::Provider;
use alloy_rpc_types_eth::{TransactionRequest, TransactionTrait};
use serde::{Deserialize, Serialize};
use magnus_primitives::{
    AASigned, SignatureType, MagnusTransaction, MagnusTxEnvelope,
    transaction::{Call, SignedKeyAuthorization, MagnusSignedAuthorization, MagnusTypedTransaction},
};

use crate::MagnusNetwork;

/// An Ethereum [`TransactionRequest`] with an optional `fee_token`.
#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    derive_more::Deref,
    derive_more::DerefMut,
)]
#[serde(rename_all = "camelCase")]
pub struct MagnusTransactionRequest {
    /// Inner [`TransactionRequest`]
    #[serde(flatten)]
    #[deref]
    #[deref_mut]
    pub inner: TransactionRequest,

    /// Optional fee token preference
    pub fee_token: Option<Address>,

    /// Optional nonce key for a 2D [`MagnusTransaction`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nonce_key: Option<U256>,

    /// Optional calls array, for Magnus transactions.
    #[serde(default)]
    pub calls: Vec<Call>,

    /// Optional key type for gas estimation of Magnus transactions.
    /// Specifies the signature verification algorithm to calculate accurate gas costs.
    pub key_type: Option<SignatureType>,

    /// Optional key-specific data for gas estimation (e.g., webauthn authenticator data).
    /// Required when key_type is WebAuthn to calculate calldata gas costs.
    pub key_data: Option<Bytes>,

    /// Optional access key ID for gas estimation.
    /// When provided, indicates the transaction uses a Keychain (access key) signature.
    /// This enables accurate gas estimation for:
    /// - Keychain signature validation overhead (+3,000 gas)
    /// - Spending limits enforcement during execution
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_id: Option<Address>,

    /// Optional authorization list for Magnus transactions (supports multiple signature types)
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        rename = "aaAuthorizationList"
    )]
    pub magnus_authorization_list: Vec<MagnusSignedAuthorization>,

    /// Key authorization for provisioning an access key (for gas estimation).
    /// Provide a signed KeyAuthorization when the transaction provisions an access key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_authorization: Option<SignedKeyAuthorization>,
}

impl MagnusTransactionRequest {
    /// Builder-pattern method for setting the fee token.
    pub fn with_fee_token(mut self, fee_token: Address) -> Self {
        self.fee_token = Some(fee_token);
        self
    }

    /// Set the 2D nonce key for the [`MagnusTransaction`] transaction.
    pub fn set_nonce_key(&mut self, nonce_key: U256) {
        self.nonce_key = Some(nonce_key);
    }

    /// Builder-pattern method for setting a 2D nonce key for a [`MagnusTransaction`].
    pub fn with_nonce_key(mut self, nonce_key: U256) -> Self {
        self.nonce_key = Some(nonce_key);
        self
    }

    /// Attempts to build a [`MagnusTransaction`] with the configured fields.
    pub fn build_aa(self) -> Result<MagnusTransaction, ValueError<Self>> {
        if self.calls.is_empty() && self.inner.to.is_none() {
            return Err(ValueError::new(
                self,
                "Missing 'calls' or 'to' field for Magnus transaction.",
            ));
        }

        let Some(nonce) = self.inner.nonce else {
            return Err(ValueError::new(
                self,
                "Missing 'nonce' field for Magnus transaction.",
            ));
        };
        let Some(gas_limit) = self.inner.gas else {
            return Err(ValueError::new(
                self,
                "Missing 'gas_limit' field for Magnus transaction.",
            ));
        };
        let Some(max_fee_per_gas) = self.inner.max_fee_per_gas else {
            return Err(ValueError::new(
                self,
                "Missing 'max_fee_per_gas' field for Magnus transaction.",
            ));
        };
        let Some(max_priority_fee_per_gas) = self.inner.max_priority_fee_per_gas else {
            return Err(ValueError::new(
                self,
                "Missing 'max_priority_fee_per_gas' field for Magnus transaction.",
            ));
        };

        let mut calls = self.calls;
        if let Some(to) = self.inner.to {
            calls.push(Call {
                to,
                value: self.inner.value.unwrap_or_default(),
                input: self.inner.input.into_input().unwrap_or_default(),
            });
        }

        Ok(MagnusTransaction {
            // TODO: use magnus mainnet chainid once assigned
            chain_id: self.inner.chain_id.unwrap_or(1),
            nonce,
            fee_payer_signature: None,
            valid_before: None,
            valid_after: None,
            gas_limit,
            max_fee_per_gas,
            max_priority_fee_per_gas,
            fee_token: self.fee_token,
            access_list: self.inner.access_list.unwrap_or_default(),
            calls,
            magnus_authorization_list: self.magnus_authorization_list,
            nonce_key: self.nonce_key.unwrap_or_default(),
            key_authorization: None,
        })
    }
}

impl AsRef<TransactionRequest> for MagnusTransactionRequest {
    fn as_ref(&self) -> &TransactionRequest {
        &self.inner
    }
}

impl AsMut<TransactionRequest> for MagnusTransactionRequest {
    fn as_mut(&mut self) -> &mut TransactionRequest {
        &mut self.inner
    }
}

impl From<TransactionRequest> for MagnusTransactionRequest {
    fn from(value: TransactionRequest) -> Self {
        Self {
            inner: value,
            fee_token: None,
            ..Default::default()
        }
    }
}

impl From<MagnusTransactionRequest> for TransactionRequest {
    fn from(value: MagnusTransactionRequest) -> Self {
        value.inner
    }
}

impl From<MagnusTxEnvelope> for MagnusTransactionRequest {
    fn from(value: MagnusTxEnvelope) -> Self {
        match value {
            MagnusTxEnvelope::Legacy(tx) => tx.into(),
            MagnusTxEnvelope::Eip2930(tx) => tx.into(),
            MagnusTxEnvelope::Eip1559(tx) => tx.into(),
            MagnusTxEnvelope::Eip7702(tx) => tx.into(),
            MagnusTxEnvelope::AA(tx) => tx.into(),
        }
    }
}

pub trait FeeToken {
    fn fee_token(&self) -> Option<Address>;
}

impl FeeToken for MagnusTransaction {
    fn fee_token(&self) -> Option<Address> {
        self.fee_token
    }
}

impl FeeToken for TxEip7702 {
    fn fee_token(&self) -> Option<Address> {
        None
    }
}

impl FeeToken for TxEip1559 {
    fn fee_token(&self) -> Option<Address> {
        None
    }
}

impl FeeToken for TxEip2930 {
    fn fee_token(&self) -> Option<Address> {
        None
    }
}

impl FeeToken for TxLegacy {
    fn fee_token(&self) -> Option<Address> {
        None
    }
}

impl<T: TransactionTrait + FeeToken> From<Signed<T>> for MagnusTransactionRequest {
    fn from(value: Signed<T>) -> Self {
        Self {
            fee_token: value.tx().fee_token(),
            inner: TransactionRequest::from_transaction(value),
            ..Default::default()
        }
    }
}

impl From<MagnusTransaction> for MagnusTransactionRequest {
    fn from(tx: MagnusTransaction) -> Self {
        Self {
            fee_token: tx.fee_token,
            inner: TransactionRequest {
                from: None,
                to: Some(tx.kind()),
                gas: Some(tx.gas_limit()),
                gas_price: tx.gas_price(),
                max_fee_per_gas: Some(tx.max_fee_per_gas()),
                max_priority_fee_per_gas: tx.max_priority_fee_per_gas(),
                value: Some(tx.value()),
                input: alloy_rpc_types_eth::TransactionInput::new(tx.input().clone()),
                nonce: Some(tx.nonce()),
                chain_id: tx.chain_id(),
                access_list: tx.access_list().cloned(),
                max_fee_per_blob_gas: None,
                blob_versioned_hashes: None,
                sidecar: None,
                authorization_list: None,
                transaction_type: Some(tx.ty()),
            },
            calls: tx.calls,
            magnus_authorization_list: tx.magnus_authorization_list,
            key_type: None,
            key_data: None,
            key_id: None,
            nonce_key: Some(tx.nonce_key),
            key_authorization: tx.key_authorization,
        }
    }
}

impl From<AASigned> for MagnusTransactionRequest {
    fn from(value: AASigned) -> Self {
        value.into_parts().0.into()
    }
}

impl From<MagnusTypedTransaction> for MagnusTransactionRequest {
    fn from(value: MagnusTypedTransaction) -> Self {
        match value {
            MagnusTypedTransaction::Legacy(tx) => Self {
                inner: tx.into(),
                fee_token: None,
                ..Default::default()
            },
            MagnusTypedTransaction::Eip2930(tx) => Self {
                inner: tx.into(),
                fee_token: None,
                ..Default::default()
            },
            MagnusTypedTransaction::Eip1559(tx) => Self {
                inner: tx.into(),
                fee_token: None,
                ..Default::default()
            },
            MagnusTypedTransaction::Eip7702(tx) => Self {
                inner: tx.into(),
                fee_token: None,
                ..Default::default()
            },
            MagnusTypedTransaction::AA(tx) => tx.into(),
        }
    }
}

/// Extension trait for [`CallBuilder`]
pub trait MagnusCallBuilderExt {
    /// Sets the `fee_token` field in the [`MagnusTransaction`] transaction to the provided value
    fn fee_token(self, fee_token: Address) -> Self;

    /// Sets the `nonce_key` field in the [`MagnusTransaction`] transaction to the provided value
    fn nonce_key(self, nonce_key: U256) -> Self;
}

impl<P: Provider<MagnusNetwork>, D: CallDecoder> MagnusCallBuilderExt
    for CallBuilder<P, D, MagnusNetwork>
{
    fn fee_token(self, fee_token: Address) -> Self {
        self.map(|request| request.with_fee_token(fee_token))
    }

    fn nonce_key(self, nonce_key: U256) -> Self {
        self.map(|request| request.with_nonce_key(nonce_key))
    }
}
