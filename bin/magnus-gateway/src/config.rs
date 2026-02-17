//! Gateway configuration.

use serde::Deserialize;
use std::path::Path;

/// Top-level gateway configuration loaded from TOML.
#[derive(Debug, Clone, Deserialize)]
pub struct GatewayConfig {
    /// Chain connection settings.
    pub chain: ChainConfig,
    /// IPFS storage settings.
    pub ipfs: IpfsConfig,
    /// REST API settings.
    pub api: ApiConfig,
    /// Connected bank endpoints.
    #[serde(default)]
    pub banks: Vec<BankConfig>,
}

/// Chain connection configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct ChainConfig {
    /// WebSocket RPC endpoint for event subscription.
    pub ws_url: String,
    /// HTTP RPC endpoint for queries.
    pub http_url: String,
    /// Chain ID for transaction signing.
    pub chain_id: u64,
}

/// IPFS node configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct IpfsConfig {
    /// IPFS API endpoint (e.g. `http://localhost:5001`).
    pub api_url: String,
    /// Gateway URL for retrieval (e.g. `http://localhost:8080`).
    pub gateway_url: String,
}

/// REST API configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiConfig {
    /// Bind address (e.g. `0.0.0.0:3000`).
    pub bind: String,
    /// API key for authentication.
    pub api_key: String,
}

/// Configuration for a connected bank.
#[derive(Debug, Clone, Deserialize)]
pub struct BankConfig {
    /// Bank identifier (BIC or custom name).
    pub id: String,
    /// Webhook URL for forwarding ISO 20022 messages.
    pub webhook_url: String,
}

impl GatewayConfig {
    /// Load configuration from a TOML file.
    pub fn load(path: impl AsRef<Path>) -> eyre::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config() {
        let toml_str = r#"
[chain]
ws_url = "wss://rpc.magnus.network"
http_url = "https://rpc.magnus.network"
chain_id = 42429

[ipfs]
api_url = "http://localhost:5001"
gateway_url = "http://localhost:8080"

[api]
bind = "0.0.0.0:3000"
api_key = "test-key"

[[banks]]
id = "VNBANK01"
webhook_url = "https://bank.example.com/webhook"
"#;

        let config: GatewayConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.chain.chain_id, 42429);
        assert_eq!(config.api.bind, "0.0.0.0:3000");
        assert_eq!(config.banks.len(), 1);
        assert_eq!(config.banks[0].id, "VNBANK01");
    }
}
