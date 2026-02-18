//! Example: Watch for incoming MIP20 transfer events in real-time.
//!
//! Connects via WebSocket and streams Transfer events as they happen.
//!
//! Requires a running Magnus node with WebSocket support.
//! Set `WS_URL` env var.
//!
//! ```bash
//! WS_URL=ws://localhost:8546 cargo run --example watch_transfers -p magnus-provider
//! ```

use alloy::primitives::{address, Address};
use alloy::providers::ProviderBuilder;
use futures::StreamExt;
use magnus_provider::{MagnusNetwork, contracts::precompiles::IMIP20};

/// Default MIP20 token address (PathUSD).
const TOKEN: Address = address!("0x20c0000000000000000000000000000000000000");

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let ws_url = std::env::var("WS_URL").expect("Set WS_URL");
    let provider = ProviderBuilder::new_with_network::<MagnusNetwork>()
        .connect(&ws_url)
        .await?;

    let token = IMIP20::new(TOKEN, &provider);

    let mut transfers = token
        .Transfer_filter()
        .watch()
        .await?
        .into_stream();

    println!("Watching for MIP20 transfers on {}...", TOKEN);
    while let Some(Ok((transfer, log))) = transfers.next().await {
        println!(
            "Transfer: {} -> {} amount={} (block {})",
            transfer.from,
            transfer.to,
            transfer.amount,
            log.block_number.unwrap_or_default()
        );
    }

    Ok(())
}
