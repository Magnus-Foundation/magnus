//! Custom transaction types for Magnus.
//!
//! Extends standard Ethereum tx types with 0x76 (Magnus Payment Transaction)
//! which adds a `fee_token` field for multi-currency gas.

use alloy_primitives::{Address, Bytes, U256};

/// Magnus-specific transaction type ID.
pub const MAGNUS_TX_TYPE_ID: u8 = 0x76;

/// A Magnus payment transaction.
///
/// Extends EIP-1559 with:
/// - `fee_token`: Address of the MIP20 token to pay gas in
/// - Standard EIP-1559 fields for EVM compatibility
#[derive(Debug, Clone)]
pub struct MagnusTransaction {
    /// Chain ID for replay protection.
    pub chain_id: u64,
    /// Transaction nonce.
    pub nonce: u64,
    /// Maximum fee per gas unit (EIP-1559).
    pub max_fee_per_gas: u128,
    /// Maximum priority fee per gas unit (EIP-1559).
    pub max_priority_fee_per_gas: u128,
    /// Gas limit for the transaction.
    pub gas_limit: u64,
    /// Transaction destination (call or create).
    pub to: alloy_primitives::TxKind,
    /// Value transferred in wei.
    pub value: U256,
    /// Transaction input data.
    pub data: Bytes,
    /// Address of the MIP20 token used to pay gas fees.
    pub fee_token: Address,
    /// EIP-2930 access list.
    pub access_list: Vec<alloy_eips::eip2930::AccessListItem>,
}

/// Decode a 0x76 Magnus transaction from RLP bytes.
///
/// Wire format: 0x76 || RLP([chain_id, nonce, max_priority_fee_per_gas,
///   max_fee_per_gas, gas_limit, to, value, data, access_list, fee_token,
///   signature_y_parity, signature_r, signature_s])
pub fn decode_magnus_tx(
    data: &[u8],
    _chain_id: u64,
) -> core::result::Result<(MagnusTransaction, Address), String> {
    // First byte must be 0x76
    if data.is_empty() || data[0] != MAGNUS_TX_TYPE_ID {
        return Err("not a Magnus transaction".into());
    }

    // TODO: Full RLP decode implementation
    // For now, return an error -- this will be implemented when
    // the RLP encoding spec is finalized
    Err("Magnus tx RLP decode not yet implemented".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magnus_tx_type_id() {
        assert_eq!(MAGNUS_TX_TYPE_ID, 0x76);
    }

    #[test]
    fn decode_rejects_wrong_type() {
        let data = [0x01, 0x00];
        assert!(decode_magnus_tx(&data, 1).is_err());
    }

    #[test]
    fn decode_rejects_empty() {
        assert!(decode_magnus_tx(&[], 1).is_err());
    }
}
