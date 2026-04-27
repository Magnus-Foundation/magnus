//! Currency registry — the on-chain allowlist of fiat currencies that are gas-eligible.
//!
//! See [`transfer-station/multi-currency-fees-design.md`] §3 / §4.3 (v3.8.2). Introduced in
//! T4 hardfork as the first step of the multi-currency fees model.
//!
//! G1 lands the storage layout, the `addCurrency` / `enableCurrency` governance setters,
//! the `validate_supported_currency` helper, and the read-only views. The disable-currency
//! hybrid (deprecation grace period + emergency disable + prune) lands in G6.
//!
//! At T4 mainnet activation, genesis seeds:
//! - `supportedCurrencies[keccak256("USD")] = { enabled: true, ... }`
//! - `supportedCurrencies[keccak256("VND")] = { enabled: true, ... }` (mainnet only — testnet is USD only)

use alloy::primitives::{B256, keccak256};
use magnus_precompiles_macros::Storable;

/// On-chain config for a registered currency.
///
/// **G1 status:** `registered` / `enabled` / `added_at_block` / `enabled_at_block` are
/// populated and used. G6 will extend this struct with deprecation fields (`deprecating: bool`,
/// `deprecation_activates_at: u64`, `last_pruned_at_block: u64`); they are NOT in this struct
/// yet because adding fields to a `Storable` struct shifts the on-disk layout. G6 may either
/// add a parallel struct or break compatibility at its hardfork boundary.
///
/// `registered` is the existence flag: `false` for any never-added currency. We can't
/// rely on `added_at_block != 0` because genesis-added currencies legitimately have
/// `added_at_block == 0`.
///
/// Storage footprint: 1 + 1 + 8 + 8 = 18 bytes, fits in a single 32-byte EVM slot.
#[derive(Clone, Debug, Default, PartialEq, Eq, Storable)]
pub struct CurrencyConfig {
    /// Whether this currency code has been added to the registry. `false` for any
    /// never-added currency. Used as the existence flag because `added_at_block == 0`
    /// is not distinguishable from "default-zero unwritten slot."
    pub registered: bool,
    /// Whether the currency is currently gas-eligible. `false` while `registered == true`
    /// means the currency is added but not yet enabled (staged rollout), OR was disabled
    /// (G6 work — deprecation grace period).
    pub enabled: bool,
    /// Block height at which `addCurrency` was called for this code.
    pub added_at_block: u64,
    /// Block height at which the currency last transitioned to `enabled = true`.
    /// Zero if the currency is registered but never enabled.
    pub enabled_at_block: u64,
}

impl CurrencyConfig {
    /// Constructs a config for a newly-added (not-yet-enabled) currency.
    pub const fn newly_added(at_block: u64) -> Self {
        Self {
            registered: true,
            enabled: false,
            added_at_block: at_block,
            enabled_at_block: 0,
        }
    }
}

/// Computes the storage map key for a given ISO 4217 code.
///
/// Solidity convention: `mapping(string => Foo)` uses `keccak256(abi.encode(string))` as the
/// per-key offset; we store under that hash so off-chain code can derive the slot identically.
pub fn currency_key(code: &str) -> B256 {
    keccak256(code.as_bytes())
}

/// Validates an ISO 4217 three-letter currency code at a basic syntactic level.
///
/// Currency codes are uppercase ASCII letters only. This is the minimal sanity check; richer
/// validation (e.g. against the actual ISO 4217 list) is intentionally out of scope — governance
/// controls which codes are registered, and the protocol cannot enforce real-world meaning.
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
        let usd_key = currency_key("USD");
        let usd_key_again = currency_key("USD");
        let vnd_key = currency_key("VND");

        assert_eq!(usd_key, usd_key_again);
        assert_ne!(usd_key, vnd_key);
        // Verify it's literally keccak256("USD")
        assert_eq!(usd_key, keccak256(b"USD"));
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
