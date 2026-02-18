//! Example: Send a MIP20 transfer with a payment memo for reconciliation.
//!
//! Requires a running Magnus node with a funded account.
//! Set `RPC_URL` and `PRIVATE_KEY` env vars.
//!
//! ```bash
//! RPC_URL=http://localhost:8545 PRIVATE_KEY=0x... cargo run --example transfer_with_memo -p magnus-provider
//! ```

use alloy::primitives::{address, Address, B256, U256};
use alloy::providers::ProviderBuilder;
use alloy::signers::local::PrivateKeySigner;
use magnus_provider::{
    MagnusNetwork,
    contracts::precompiles::IMIP20,
    provider::ext::MagnusProviderBuilderExt,
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

    let token = IMIP20::new(TOKEN, &provider);

    let recipient = address!("0x742d35Cc6634C0532925a3b844Bc9e7595f0bEbb");
    let amount = U256::from(100_000_000); // 100 tokens (6 decimals)
    let memo = B256::left_padding_from(b"INV-2026-00142");

    let receipt = token
        .transferWithMemo(recipient, amount, memo)
        .send()
        .await?
        .get_receipt()
        .await?;

    println!("Transfer tx: {:?}", receipt.transaction_hash);
    Ok(())
}
