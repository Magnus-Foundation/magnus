//! Magnus Banking Gateway — bridges on-chain stablecoin payments to ISO 20022 banking messages.
#![warn(missing_docs)]

mod api;
mod chain;
mod config;
mod iso20022;
mod storage;

use config::GatewayConfig;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "magnus_gateway=info".into()),
        )
        .init();

    let config_path =
        std::env::args().nth(1).unwrap_or_else(|| "gateway.toml".to_string());

    let config = GatewayConfig::load(&config_path)?;
    tracing::info!(
        bind = %config.api.bind,
        chain_id = config.chain.chain_id,
        "Magnus Banking Gateway starting"
    );

    // Start chain listener and API server concurrently
    let chain_config = config.chain.clone();
    let api_config = config.clone();

    tokio::select! {
        result = chain::listener::run(chain_config) => {
            tracing::error!(?result, "Chain listener exited");
            result
        }
        result = api::server::run(api_config) => {
            tracing::error!(?result, "API server exited");
            result
        }
    }
}
