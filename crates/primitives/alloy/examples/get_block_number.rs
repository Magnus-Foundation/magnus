//! Get the current block number from the Magnus network.
//!
//! Run with: `cargo run --example get_block_number`

use alloy::providers::{Provider, ProviderBuilder};
use magnus_alloy::MagnusNetwork;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = ProviderBuilder::new_with_network::<MagnusNetwork>()
        .connect(&std::env::var("RPC_URL").expect("No RPC URL set"))
        .await?;

    println!("{}", provider.get_block_number().await?);

    Ok(())
}
