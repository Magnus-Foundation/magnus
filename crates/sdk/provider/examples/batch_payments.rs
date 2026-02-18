//! Example: Atomic batch payment — multiple MIP20 transfers in a single transaction.
//!
//! Uses Magnus's native multi-call transaction type to batch several transfers
//! atomically. All succeed or all revert.
//!
//! Requires a running Magnus node with a funded account.
//! Set `RPC_URL` and `PRIVATE_KEY` env vars.
//!
//! ```bash
//! RPC_URL=http://localhost:8545 PRIVATE_KEY=0x... cargo run --example batch_payments -p magnus-provider
//! ```

use alloy::primitives::{address, Address, TxKind, U256};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::signers::local::PrivateKeySigner;
use alloy::sol_types::SolCall;
use magnus_provider::{
    MagnusNetwork,
    contracts::precompiles::IMIP20,
    primitives::transaction::Call,
    provider::ext::MagnusProviderBuilderExt,
    rpc::MagnusTransactionRequest,
};

/// Default MIP20 token address (PathUSD).
const TOKEN: Address = address!("0x20c0000000000000000000000000000000000000");

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let rpc_url = std::env::var("RPC_URL").expect("Set RPC_URL");
    let signer: PrivateKeySigner = std::env::var("PRIVATE_KEY")
        .expect("Set PRIVATE_KEY")
        .parse()?;

    let provider = ProviderBuilder::new_with_network::<MagnusNetwork>()
        .with_random_2d_nonces()
        .wallet(signer)
        .connect(&rpc_url)
        .await?;

    // Build batch: pay 3 recipients atomically.
    let recipients: Vec<(Address, U256)> = vec![
        (address!("0xAA00000000000000000000000000000000000001"), U256::from(50_000_000)),
        (address!("0xBB00000000000000000000000000000000000002"), U256::from(75_000_000)),
        (address!("0xCC00000000000000000000000000000000000003"), U256::from(25_000_000)),
    ];

    let calls: Vec<Call> = recipients
        .iter()
        .map(|(to, amount)| Call {
            to: TxKind::Call(TOKEN),
            value: U256::ZERO,
            input: IMIP20::transferCall {
                to: *to,
                amount: *amount,
            }
            .abi_encode()
            .into(),
        })
        .collect();

    let receipt = provider
        .send_transaction(MagnusTransactionRequest {
            calls,
            ..Default::default()
        })
        .await?
        .get_receipt()
        .await?;

    println!("Batch payment tx: {:?}", receipt.transaction_hash);
    println!("Paid {} recipients atomically", recipients.len());
    Ok(())
}
