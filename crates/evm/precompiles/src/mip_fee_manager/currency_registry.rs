//! Currency registry — the on-chain allowlist of fiat currencies that are gas-eligible.
//!
//! See [`transfer-station/multi-currency-fees-design.md`] §3 / §4.3 (v3.8.2). Introduced in
//! T4 hardfork as the first step of the multi-currency fees model.
//!
//! **G0 status:** scaffold only — types defined, storage layout reserved on FeeManager,
//! governance functions (`addCurrency`, `enableCurrency`, `disableCurrency`) land in G1.
//! At v3.8.2 §11.1 the disable-currency hybrid (deprecation + emergency + prune) is also
//! tracked here; that detail lands in G6.
//!
//! At T4 mainnet activation, genesis seeds:
//! - `supportedCurrencies["USD"] = { enabled: true, ... }`
//! - `supportedCurrencies["VND"] = { enabled: true, ... }` (mainnet only — testnet is USD only)

use alloy::sol;

sol! {
    /// On-chain config for a registered currency.
    ///
    /// **G0:** layout reserved. Fields beyond `enabled`/`addedAtBlock`/`enabledAtBlock` land
    /// in G6 to back the disable-currency hybrid (`deprecating`, `deprecationActivatesAt`,
    /// `lastPrunedAtBlock`).
    #[derive(Debug)]
    struct CurrencyConfig {
        bool   enabled;
        uint64 addedAtBlock;
        uint64 enabledAtBlock;
    }
}

/// Validates an ISO 4217 three-letter currency code at a basic syntactic level.
///
/// **G0:** length check only. G1 may add a stricter alphanumeric check.
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
}
