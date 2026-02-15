//! Custom transaction types for Magnus.
//!
//! Extends standard Ethereum tx types with 0x76 (Magnus Payment Transaction)
//! which adds a `fee_token` field for multi-currency gas and `calls` for
//! batched execution.

use alloy_primitives::{Address, Bytes, B256, U256, TxKind};
use alloy_rlp::{BufMut, Decodable, Encodable};
use alloy_eips::eip2930::AccessListItem;

/// Magnus-specific transaction type ID.
pub const MAGNUS_TX_TYPE_ID: u8 = 0x76;

/// A single call within a Magnus transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    /// Destination: call or create.
    pub to: TxKind,
    /// Value in wei.
    pub value: U256,
    /// Calldata.
    pub input: Bytes,
}

impl Encodable for Call {
    fn encode(&self, out: &mut dyn BufMut) {
        let header = alloy_rlp::Header {
            list: true,
            payload_length: self.to.length() + self.value.length() + self.input.length(),
        };
        header.encode(out);
        self.to.encode(out);
        self.value.encode(out);
        self.input.encode(out);
    }

    fn length(&self) -> usize {
        let payload = self.to.length() + self.value.length() + self.input.length();
        payload + alloy_rlp::length_of_length(payload)
    }
}

impl Decodable for Call {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let header = alloy_rlp::Header::decode(buf)?;
        if !header.list {
            return Err(alloy_rlp::Error::UnexpectedString);
        }
        Ok(Self {
            to: Decodable::decode(buf)?,
            value: Decodable::decode(buf)?,
            input: Decodable::decode(buf)?,
        })
    }
}

/// Magnus transaction type (0x76).
///
/// Extends EIP-1559 with:
/// - fee_token: MIP20 token address for gas payment
/// - calls: batch of calls (Vec<Call>)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MagnusTransaction {
    /// Chain ID for replay protection.
    pub chain_id: u64,
    /// Transaction nonce.
    pub nonce: u64,
    /// Maximum priority fee per gas unit (EIP-1559).
    pub max_priority_fee_per_gas: u128,
    /// Maximum fee per gas unit (EIP-1559).
    pub max_fee_per_gas: u128,
    /// Gas limit for the transaction.
    pub gas_limit: u64,
    /// Batched calls.
    pub calls: Vec<Call>,
    /// EIP-2930 access list.
    pub access_list: Vec<AccessListItem>,
    /// Address of the MIP20 token used to pay gas fees (None = native).
    pub fee_token: Option<Address>,
    /// Value transferred in wei.
    pub value: U256,
    /// Transaction input data.
    pub data: Bytes,
}

impl MagnusTransaction {
    /// Encode the transaction fields as an RLP list (without the type byte).
    fn rlp_encode_fields(&self, out: &mut dyn BufMut) {
        self.chain_id.encode(out);
        self.max_priority_fee_per_gas.encode(out);
        self.max_fee_per_gas.encode(out);
        self.gas_limit.encode(out);

        // Encode calls as RLP list
        let calls_header = alloy_rlp::Header {
            list: true,
            payload_length: self.calls.iter().map(|c| c.length()).sum(),
        };
        calls_header.encode(out);
        for call in &self.calls {
            call.encode(out);
        }

        // Access list
        let al = alloy_eips::eip2930::AccessList(
            self.access_list.clone(),
        );
        al.encode(out);

        self.nonce.encode(out);

        // Optional fee_token
        match self.fee_token {
            Some(addr) => addr.encode(out),
            None => out.put_u8(0x80), // RLP empty string
        }
    }

    fn rlp_fields_length(&self) -> usize {
        let calls_payload: usize = self.calls.iter().map(|c| c.length()).sum();
        let calls_len = calls_payload + alloy_rlp::length_of_length(calls_payload);

        let al = alloy_eips::eip2930::AccessList(
            self.access_list.clone(),
        );

        self.chain_id.length()
            + self.max_priority_fee_per_gas.length()
            + self.max_fee_per_gas.length()
            + self.gas_limit.length()
            + calls_len
            + al.length()
            + self.nonce.length()
            + self.fee_token.map_or(1, |a| a.length()) // 1 byte for 0x80 if None
    }

    /// Compute the signing hash for this transaction.
    pub fn signature_hash(&self) -> B256 {
        use alloy_primitives::keccak256;
        let mut buf = Vec::with_capacity(1 + self.rlp_fields_length() + 5);
        buf.put_u8(MAGNUS_TX_TYPE_ID);
        let header = alloy_rlp::Header {
            list: true,
            payload_length: self.rlp_fields_length(),
        };
        header.encode(&mut buf);
        self.rlp_encode_fields(&mut buf);
        keccak256(&buf)
    }
}

/// Decode a Magnus transaction from bytes.
///
/// Format: 0x76 || RLP([chain_id, max_priority_fee_per_gas, max_fee_per_gas,
///         gas_limit, calls, access_list, nonce, fee_token])
pub fn decode_magnus_tx(
    data: &[u8],
    chain_id: u64,
) -> core::result::Result<MagnusTransaction, String> {
    if data.is_empty() {
        return Err("empty transaction data".into());
    }
    if data[0] != MAGNUS_TX_TYPE_ID {
        return Err(format!("expected type 0x{:02x}, got 0x{:02x}", MAGNUS_TX_TYPE_ID, data[0]));
    }

    let mut buf = &data[1..]; // Skip type byte
    let header = alloy_rlp::Header::decode(&mut buf)
        .map_err(|e| format!("RLP header: {}", e))?;
    if !header.list {
        return Err("expected RLP list".into());
    }

    let decoded_chain_id: u64 = Decodable::decode(&mut buf)
        .map_err(|e| format!("chain_id: {}", e))?;
    if decoded_chain_id != chain_id {
        return Err(format!("chain_id mismatch: expected {}, got {}", chain_id, decoded_chain_id));
    }

    let max_priority_fee_per_gas: u128 = Decodable::decode(&mut buf)
        .map_err(|e| format!("max_priority_fee: {}", e))?;
    let max_fee_per_gas: u128 = Decodable::decode(&mut buf)
        .map_err(|e| format!("max_fee: {}", e))?;
    let gas_limit: u64 = Decodable::decode(&mut buf)
        .map_err(|e| format!("gas_limit: {}", e))?;

    // Decode calls
    let calls_header = alloy_rlp::Header::decode(&mut buf)
        .map_err(|e| format!("calls header: {}", e))?;
    let mut calls = Vec::new();
    let mut calls_buf = &buf[..calls_header.payload_length];
    while !calls_buf.is_empty() {
        let call = Call::decode(&mut calls_buf)
            .map_err(|e| format!("call: {}", e))?;
        calls.push(call);
    }
    buf = &buf[calls_header.payload_length..];

    let access_list: alloy_eips::eip2930::AccessList = Decodable::decode(&mut buf)
        .map_err(|e| format!("access_list: {}", e))?;

    let nonce: u64 = Decodable::decode(&mut buf)
        .map_err(|e| format!("nonce: {}", e))?;

    // Optional fee_token
    let fee_token = if !buf.is_empty() && buf[0] == 0x80 {
        buf = &buf[1..]; // Skip empty byte
        None
    } else if !buf.is_empty() {
        let addr: Address = Decodable::decode(&mut buf)
            .map_err(|e| format!("fee_token: {}", e))?;
        Some(addr)
    } else {
        None
    };

    Ok(MagnusTransaction {
        chain_id: decoded_chain_id,
        nonce,
        max_priority_fee_per_gas,
        max_fee_per_gas,
        gas_limit,
        calls,
        access_list: access_list.0,
        fee_token,
        value: U256::ZERO,
        data: Bytes::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::Address;

    #[test]
    fn magnus_tx_type_id() {
        assert_eq!(MAGNUS_TX_TYPE_ID, 0x76);
    }

    #[test]
    fn call_rlp_roundtrip() {
        let call = Call {
            to: TxKind::Call(Address::with_last_byte(1)),
            value: U256::from(100),
            input: Bytes::from(vec![0xab, 0xcd]),
        };
        let mut buf = Vec::new();
        call.encode(&mut buf);
        let decoded = Call::decode(&mut buf.as_slice()).unwrap();
        assert_eq!(decoded, call);
    }

    #[test]
    fn magnus_tx_encode_decode_roundtrip() {
        let tx = MagnusTransaction {
            chain_id: 42,
            nonce: 5,
            max_priority_fee_per_gas: 1_000_000_000,
            max_fee_per_gas: 2_000_000_000,
            gas_limit: 21000,
            calls: vec![Call {
                to: TxKind::Call(Address::with_last_byte(0xAA)),
                value: U256::from(1000),
                input: Bytes::new(),
            }],
            access_list: vec![],
            fee_token: Some(Address::with_last_byte(0xFE)),
            value: U256::ZERO,
            data: Bytes::new(),
        };

        // Encode
        let mut buf = Vec::new();
        buf.push(MAGNUS_TX_TYPE_ID);
        let header = alloy_rlp::Header {
            list: true,
            payload_length: tx.rlp_fields_length(),
        };
        header.encode(&mut buf);
        tx.rlp_encode_fields(&mut buf);

        // Decode
        let decoded = decode_magnus_tx(&buf, 42).unwrap();
        assert_eq!(decoded.chain_id, 42);
        assert_eq!(decoded.nonce, 5);
        assert_eq!(decoded.gas_limit, 21000);
        assert_eq!(decoded.fee_token, Some(Address::with_last_byte(0xFE)));
        assert_eq!(decoded.calls.len(), 1);
    }

    #[test]
    fn magnus_tx_no_fee_token() {
        let tx = MagnusTransaction {
            chain_id: 1,
            nonce: 0,
            max_priority_fee_per_gas: 0,
            max_fee_per_gas: 0,
            gas_limit: 21000,
            calls: vec![],
            access_list: vec![],
            fee_token: None,
            value: U256::ZERO,
            data: Bytes::new(),
        };

        let mut buf = Vec::new();
        buf.push(MAGNUS_TX_TYPE_ID);
        let header = alloy_rlp::Header {
            list: true,
            payload_length: tx.rlp_fields_length(),
        };
        header.encode(&mut buf);
        tx.rlp_encode_fields(&mut buf);

        let decoded = decode_magnus_tx(&buf, 1).unwrap();
        assert_eq!(decoded.fee_token, None);
    }

    #[test]
    fn decode_rejects_wrong_type() {
        let data = [0x01, 0xc0];
        assert!(decode_magnus_tx(&data, 1).is_err());
    }

    #[test]
    fn decode_rejects_empty() {
        assert!(decode_magnus_tx(&[], 1).is_err());
    }

    #[test]
    fn signature_hash_deterministic() {
        let tx = MagnusTransaction {
            chain_id: 1,
            nonce: 0,
            max_priority_fee_per_gas: 0,
            max_fee_per_gas: 1_000_000_000,
            gas_limit: 21000,
            calls: vec![],
            access_list: vec![],
            fee_token: None,
            value: U256::ZERO,
            data: Bytes::new(),
        };
        let h1 = tx.signature_hash();
        let h2 = tx.signature_hash();
        assert_eq!(h1, h2);
        assert_ne!(h1, B256::ZERO);
    }
}
