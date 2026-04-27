//! On-chain allowlist of fiat currencies that are gas-eligible.
//!
//! See `transfer-station/multi-currency-fees-design.md` §3, §4.3.

use alloy::primitives::{B256, keccak256};
use magnus_precompiles_macros::Storable;

/// On-chain config for a registered currency. Storage footprint: 18 bytes (1 slot).
///
/// `registered` is the existence flag — `false` for any never-added code (a default
/// zero slot is indistinguishable from "added at block 0").
#[derive(Clone, Debug, Default, PartialEq, Eq, Storable)]
pub struct CurrencyConfig {
    pub registered: bool,
    pub enabled: bool,
    pub added_at_block: u64,
    pub enabled_at_block: u64,
}

impl CurrencyConfig {
    pub const fn newly_added(at_block: u64) -> Self {
        Self {
            registered: true,
            enabled: false,
            added_at_block: at_block,
            enabled_at_block: 0,
        }
    }
}

/// Storage key for `supportedCurrencies`. Mirrors Solidity's `mapping(string => Foo)`.
pub fn currency_key(code: &str) -> B256 {
    keccak256(code.as_bytes())
}

/// Validates an ISO 4217 three-letter currency code (3 uppercase ASCII letters).
pub fn is_valid_currency_code(code: &str) -> bool {
    code.len() == 3 && code.chars().all(|c| c.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_4217_three_letter_uppercase_codes_are_valid() {
        assert!(is_valid_currency_code("USD"));
        assert!(is_valid_currency_code("VND"));
        assert!(is_valid_currency_code("EUR"));
        assert!(is_valid_currency_code("GBP"));
    }

    #[test]
    fn lowercase_or_wrong_length_codes_are_invalid() {
        assert!(!is_valid_currency_code("usd"));
        assert!(!is_valid_currency_code("US"));
        assert!(!is_valid_currency_code("USDT"));
        assert!(!is_valid_currency_code(""));
        assert!(!is_valid_currency_code("US1"));
    }

    #[test]
    fn currency_key_is_deterministic_keccak256() {
        assert_eq!(currency_key("USD"), keccak256(b"USD"));
        assert_eq!(currency_key("USD"), currency_key("USD"));
        assert_ne!(currency_key("USD"), currency_key("VND"));
    }

    #[test]
    fn currency_config_default_is_unregistered_and_disabled() {
        let config = CurrencyConfig::default();
        assert!(!config.registered);
        assert!(!config.enabled);
        assert_eq!(config.added_at_block, 0);
        assert_eq!(config.enabled_at_block, 0);
    }

    #[test]
    fn newly_added_constructor_marks_registered_at_genesis_block() {
        let config = CurrencyConfig::newly_added(0);
        assert!(config.registered);
        assert!(!config.enabled);
        assert_eq!(config.added_at_block, 0);
        assert_eq!(config.enabled_at_block, 0);
    }

    #[test]
    fn newly_added_with_nonzero_block() {
        let config = CurrencyConfig::newly_added(42);
        assert!(config.registered);
        assert!(!config.enabled);
        assert_eq!(config.added_at_block, 42);
        assert_eq!(config.enabled_at_block, 0);
    }
}
