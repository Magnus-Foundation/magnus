# Magnus Provider

Magnus types for [Alloy](https://alloy.rs).

## Getting Started

To use `magnus-provider`, add the crate as a dependency in your `Cargo.toml` file:

```toml
[dependencies]
magnus-provider = { path = "crates/sdk/provider" }
```

## Development Status

`magnus-provider` is currently in development.

## Usage

To get started, instantiate a provider with [`MagnusNetwork`]:

```rust
use alloy::{
    providers::{Provider, ProviderBuilder},
    transports::TransportError
};
use magnus_provider::MagnusNetwork;

async fn build_provider() -> Result<impl Provider<MagnusNetwork>, TransportError> {
    ProviderBuilder::new_with_network::<MagnusNetwork>()
        .connect("https://rpc.example.com")
        .await
}
```

This crate also exposes bindings for all Magnus precompiles, such as MIP20:

```rust,ignore
use alloy::{
    primitives::{U256, address},
    providers::ProviderBuilder,
};
use magnus_provider::{MagnusNetwork, contracts::precompiles::IMIP20};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = ProviderBuilder::new_with_network::<MagnusNetwork>()
        .connect(&std::env::var("RPC_URL").expect("No RPC URL set"))
        .await?;

    let token = IMIP20::new(
        address!("0x20c0000000000000000000000000000000000001"),
        provider,
    );

    let receipt = token
        .transfer(
            address!("0x742d35Cc6634C0532925a3b844Bc9e7595f0bEbb"),
            U256::from(100).pow(U256::from(10e6)),
        )
        .send()
        .await?
        .get_receipt()
        .await?;

    Ok(())
}
```
