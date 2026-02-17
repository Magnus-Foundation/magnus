//! WebSocket event listener for on-chain payment events.

use crate::{chain::client::PaymentEvent, config::ChainConfig};
use alloy::primitives::{Address, B256};
use eyre::Result;

/// Run the chain event listener loop.
///
/// Subscribes to new block headers via WebSocket and scans for
/// `TransferWithPaymentData` events from MIP-20 tokens.
pub async fn run(config: ChainConfig) -> Result<()> {
    tracing::info!(ws_url = %config.ws_url, "Starting chain listener");

    // In production, this would:
    // 1. Connect to the WS endpoint
    // 2. Subscribe to newHeads
    // 3. For each block, fetch logs matching TransferWithPaymentData topic
    // 4. Parse logs into PaymentEvent structs
    // 5. Forward to storage + ISO 20022 pipeline

    // For MVP, we poll using HTTP
    let mut last_block: u64 = 0;

    loop {
        match poll_new_events(&config.http_url, &mut last_block).await {
            Ok(events) => {
                for event in events {
                    tracing::info!(
                        tx = %event.tx_hash,
                        e2e_id = %event.end_to_end_id,
                        "Payment event detected"
                    );
                    // TODO: forward to storage and ISO 20022 pipeline
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to poll events, retrying...");
            }
        }

        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}

/// Poll for new payment events since `last_block`.
async fn poll_new_events(http_url: &str, last_block: &mut u64) -> Result<Vec<PaymentEvent>> {
    let client = reqwest::Client::new();

    // Get current block number
    let resp = client
        .post(http_url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_blockNumber",
            "params": [],
            "id": 1
        }))
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;

    let current_block = resp
        .get("result")
        .and_then(|v| v.as_str())
        .map(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).unwrap_or(0))
        .unwrap_or(0);

    if current_block <= *last_block {
        return Ok(vec![]);
    }

    // Query logs for TransferWithPaymentData events
    // Topic0 = keccak256("TransferWithPaymentData(address,address,uint256,string,string,string,bytes32)")
    let from_block = format!("0x{:x}", *last_block + 1);
    let to_block = format!("0x{:x}", current_block);

    let _resp = client
        .post(http_url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getLogs",
            "params": [{
                "fromBlock": from_block,
                "toBlock": to_block,
                // TODO: add specific topic filter for TransferWithPaymentData
            }],
            "id": 2
        }))
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;

    *last_block = current_block;

    // TODO: parse logs into PaymentEvent structs
    // For now, return empty — actual log parsing will be implemented
    // when the MIP-20 TransferWithPaymentData event ABI is finalized
    Ok(vec![])
}

/// Parse a raw log entry into a `PaymentEvent`.
///
/// This extracts the indexed parameters (from, to) from topics
/// and the non-indexed parameters from the data field.
pub fn parse_payment_log(
    tx_hash: B256,
    block_number: u64,
    token: Address,
    _topics: &[B256],
    _data: &[u8],
) -> Option<PaymentEvent> {
    // TODO: implement actual ABI decoding
    // topics[1] = from (indexed)
    // topics[2] = to (indexed)
    // data = abi.encode(amount, endToEndId, purposeCode, remittanceInfo, messageHash)

    Some(PaymentEvent {
        tx_hash,
        block_number,
        token,
        from: Address::ZERO,
        to: Address::ZERO,
        amount: "0".to_string(),
        end_to_end_id: String::new(),
        purpose_code: String::new(),
        remittance_info: String::new(),
        message_hash: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_payment_log_placeholder() {
        let event = parse_payment_log(
            B256::ZERO,
            100,
            Address::ZERO,
            &[],
            &[],
        );
        assert!(event.is_some());
        let event = event.unwrap();
        assert_eq!(event.block_number, 100);
    }
}
