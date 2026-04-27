//! On-chain allowlist of fiat currencies that are gas-eligible.
//!
//! See `transfer-station/multi-currency-fees-design.md` §3, §4.3.

use alloy::primitives::{B256, keccak256};
use magnus_precompiles_macros::Storable;

/// On-chain config for a registered currency.
///
/// `registered` is the existence flag — `false` for any never-added code (a default
/// zero slot is indistinguishable from "added at block 0").
///
/// `deprecating` + `deprecation_activates_at` model the standard-disable grace
/// period: while deprecating, `enabled` stays true but new factory deploys and
/// new validator accept-set adds are blocked. After the grace timestamp, lazy
/// reads treat the currency as disabled.
#[derive(Clone, Debug, Default, PartialEq, Eq, Storable)]
pub struct CurrencyConfig {
    pub registered: bool,
    pub enabled: bool,
    pub deprecating: bool,
    pub added_at_block: u64,
    pub enabled_at_block: u64,
    pub deprecation_activates_at: u64,
    pub last_pruned_at_block: u64,
}

impl CurrencyConfig {
    pub const fn newly_added(at_block: u64) -> Self {
        Self {
            registered: true,
            enabled: false,
            deprecating: false,
            added_at_block: at_block,
            enabled_at_block: 0,
            deprecation_activates_at: 0,
            last_pruned_at_block: 0,
        }
    }

    /// Effective enabled state at `now_ts`: `enabled` AND not past the grace boundary.
    pub fn effectively_enabled(&self, now_ts: u64) -> bool {
        if !self.enabled {
            return false;
        }
        if self.deprecating && now_ts >= self.deprecation_activates_at {
            return false;
        }
        true
    }

    /// True iff the currency is in the grace window (deprecating but not yet expired).
    pub fn in_grace_period(&self, now_ts: u64) -> bool {
        self.deprecating && now_ts < self.deprecation_activates_at
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
        assert!(!config.deprecating);
        assert_eq!(config.added_at_block, 0);
        assert_eq!(config.enabled_at_block, 0);
        assert_eq!(config.deprecation_activates_at, 0);
        assert_eq!(config.last_pruned_at_block, 0);
    }

    #[test]
    fn newly_added_constructor_marks_registered_at_genesis_block() {
        let config = CurrencyConfig::newly_added(0);
        assert!(config.registered);
        assert!(!config.enabled);
        assert!(!config.deprecating);
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

    #[test]
    fn effectively_enabled_handles_grace_window() {
        let mut cfg = CurrencyConfig::newly_added(0);
        cfg.enabled = true;
        assert!(cfg.effectively_enabled(0));
        assert!(cfg.effectively_enabled(u64::MAX));

        cfg.deprecating = true;
        cfg.deprecation_activates_at = 1_000;
        assert!(cfg.effectively_enabled(999));
        assert!(!cfg.effectively_enabled(1_000));
        assert!(!cfg.effectively_enabled(u64::MAX));

        assert!(cfg.in_grace_period(999));
        assert!(!cfg.in_grace_period(1_000));
    }

    #[test]
    fn effectively_enabled_returns_false_when_enabled_is_false() {
        let cfg = CurrencyConfig::newly_added(0);
        assert!(!cfg.effectively_enabled(0));
    }
}
