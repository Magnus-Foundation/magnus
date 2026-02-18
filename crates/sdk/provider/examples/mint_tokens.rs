//! Example: Mint new MIP20 tokens (requires ISSUER_ROLE).
//!
//! The signer must have the ISSUER_ROLE on the target token.
//!
//! Requires a running Magnus node.
//! Set `RPC_URL` and `PRIVATE_KEY` env vars.
//!
//! ```bash
//! RPC_URL=http://localhost:8545 PRIVATE_KEY=0x... cargo run --example mint_tokens -p magnus-provider
//! ```

use alloy::primitives::{address, Address, U256};
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
    let amount = U256::from(1_000_000_000); // 1000 tokens (6 decimals)

    let receipt = token
        .mint(recipient, amount)
        .send()
        .await?
        .get_receipt()
        .await?;

    println!("Mint tx: {:?}", receipt.transaction_hash);
    println!("Minted {} to {}", amount, recipient);
    Ok(())
}
