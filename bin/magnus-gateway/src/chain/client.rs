//! Thin RPC client wrapper for querying chain data.

use alloy::primitives::{Address, B256};
use eyre::Result;

/// RPC client for querying the Magnus chain.
#[derive(Debug, Clone)]
pub struct ChainClient {
    http_url: String,
}

/// On-chain payment event data extracted from `TransferWithPaymentData` logs.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PaymentEvent {
    /// The on-chain transaction hash.
    pub tx_hash: B256,
    /// Block number where the event was emitted.
    pub block_number: u64,
    /// Token contract address.
    pub token: Address,
    /// Sender address.
    pub from: Address,
    /// Recipient address.
    pub to: Address,
    /// Transfer amount (raw, token-decimals).
    pub amount: String,
    /// ISO 20022 end-to-end ID embedded in the transfer.
    pub end_to_end_id: String,
    /// ISO 20022 purpose code.
    pub purpose_code: String,
    /// Free-text remittance information.
    pub remittance_info: String,
    /// IPFS hash of the full ISO 20022 message (if attached).
    pub message_hash: Option<String>,
}

impl ChainClient {
    /// Create a new chain client.
    pub fn new(http_url: String) -> Self {
        Self { http_url }
    }

    /// Returns the HTTP RPC URL.
    pub fn http_url(&self) -> &str {
        &self.http_url
    }

    /// Query a transaction receipt by hash.
    ///
    /// Returns the raw JSON response from the RPC node.
    pub async fn get_transaction_receipt(&self, tx_hash: B256) -> Result<Option<serde_json::Value>> {
        let client = reqwest::Client::new();
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getTransactionReceipt",
            "params": [format!("{tx_hash:#x}")],
            "id": 1
        });

        let resp = client
            .post(&self.http_url)
            .json(&body)
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        Ok(resp.get("result").cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_payment_event_serialization() {
        let event = PaymentEvent {
            tx_hash: B256::ZERO,
            block_number: 42,
            token: Address::ZERO,
            from: Address::ZERO,
            to: Address::ZERO,
            amount: "1000000".to_string(),
            end_to_end_id: "E2E-001".to_string(),
            purpose_code: "SALA".to_string(),
            remittance_info: "January salary".to_string(),
            message_hash: Some("QmTest123".to_string()),
        };

        let json = serde_json::to_string(&event).unwrap();
        let decoded: PaymentEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.end_to_end_id, "E2E-001");
        assert_eq!(decoded.block_number, 42);
    }
}
