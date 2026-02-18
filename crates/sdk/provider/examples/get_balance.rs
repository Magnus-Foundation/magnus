//! Example: Query MIP20 token balance for an address.
//!
//! Requires a running Magnus node. Set `RPC_URL` env var.
//!
//! ```bash
//! RPC_URL=http://localhost:8545 cargo run --example get_balance -p magnus-provider
//! ```

use alloy::primitives::{address, Address};
use alloy::providers::ProviderBuilder;
use magnus_provider::{MagnusNetwork, contracts::precompiles::IMIP20};

/// Default MIP20 token address (PathUSD).
const TOKEN: Address = address!("0x20c0000000000000000000000000000000000000");

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let rpc_url = std::env::var("RPC_URL").expect("Set RPC_URL");
    let provider = ProviderBuilder::new_with_network::<MagnusNetwork>()
        .connect(&rpc_url)
        .await?;

    let token = IMIP20::new(TOKEN, &provider);

    let account = address!("0x742d35Cc6634C0532925a3b844Bc9e7595f0bEbb");
    let balance = token.balanceOf(account).call().await?;

    println!("Balance of {account}: {balance}");
    Ok(())
}
