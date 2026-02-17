//! Oracle Registry -- manages FX rate feeds for multi-currency gas.
//!
//! **DEPRECATED**: This is the prototype in-memory implementation.
//! Use [`magnus_precompile_registry::oracle_registry`] for the production
//! EVM precompile with persistent storage.
//!
//! Implements the Celo SortedOracles pattern with extensions:
//! - Multiple rate pairs (e.g., VND/USD, EUR/USD, NGN/USD)
//! - Whitelisted reporters (validators + external oracles)
//! - BreakerBox circuit breaker for extreme rate movements
//! - Report expiry (default 360 seconds, configurable per pair)
//!
//! Rate pairs are identified by (base_token, quote_token) addresses.
//! The rate represents: 1 unit of base_token = rate units of quote_token.

pub mod sorted_list;

use alloy_primitives::{Address, B256, U256, keccak256};
use sorted_list::SortedOracleList;
use std::collections::HashMap;

use crate::{
    addresses,
    error::{MagnusPrecompileError, Result},
};

/// Default report expiry in seconds (from Celo).
pub const DEFAULT_REPORT_EXPIRY: u64 = 360;

/// Maximum allowed rate deviation from median before circuit breaker trips.
/// 20% deviation triggers the breaker.
pub const BREAKER_THRESHOLD_BPS: u64 = 2000; // 20% in basis points
/// Basis points denominator.
pub const BPS_DENOMINATOR: u64 = 10000;

/// Rate pair identifier.
pub fn rate_pair_id(base_token: Address, quote_token: Address) -> B256 {
    let mut data = Vec::with_capacity(40);
    data.extend_from_slice(base_token.as_slice());
    data.extend_from_slice(quote_token.as_slice());
    keccak256(data)
}

/// Oracle Registry state.
#[derive(Debug)]
#[deprecated(note = "Use magnus_precompile_registry::oracle_registry::OracleRegistry instead")]
pub struct OracleRegistry {
    /// The precompile address of the oracle registry.
    pub address: Address,
    /// Rate pair -> sorted oracle list
    rate_pairs: HashMap<B256, SortedOracleList>,
    /// Whitelisted reporters
    reporters: HashMap<Address, bool>,
    /// Per-pair expiry overrides
    expiry_overrides: HashMap<B256, u64>,
    /// Circuit breaker: pairs that are currently frozen
    frozen_pairs: HashMap<B256, bool>,
}

impl OracleRegistry {
    /// Create a new OracleRegistry.
    pub fn new() -> Self {
        Self {
            address: addresses::ORACLE_REGISTRY,
            rate_pairs: HashMap::new(),
            reporters: HashMap::new(),
            expiry_overrides: HashMap::new(),
            frozen_pairs: HashMap::new(),
        }
    }

    /// Add a whitelisted reporter.
    pub fn add_reporter(&mut self, reporter: Address) {
        self.reporters.insert(reporter, true);
    }

    /// Remove a reporter.
    pub fn remove_reporter(&mut self, reporter: Address) {
        self.reporters.remove(&reporter);
    }

    /// Check if an address is a whitelisted reporter.
    pub fn is_reporter(&self, addr: &Address) -> bool {
        self.reporters.get(addr).copied().unwrap_or(false)
    }

    /// Submit a rate report.
    ///
    /// Only whitelisted reporters can submit. The report is inserted
    /// into the sorted list for the rate pair.
    pub fn report(
        &mut self,
        reporter: Address,
        base_token: Address,
        quote_token: Address,
        value: U256,
        timestamp: u64,
    ) -> Result<()> {
        if !self.is_reporter(&reporter) {
            return Err(MagnusPrecompileError::Unauthorized(
                "not a whitelisted reporter".into(),
            ));
        }

        let pair_id = rate_pair_id(base_token, quote_token);

        // Check circuit breaker
        if self.frozen_pairs.get(&pair_id).copied().unwrap_or(false) {
            return Err(MagnusPrecompileError::Oracle(
                "rate pair is frozen by circuit breaker".into(),
            ));
        }

        let expiry = self
            .expiry_overrides
            .get(&pair_id)
            .copied()
            .unwrap_or(DEFAULT_REPORT_EXPIRY);

        let list = self
            .rate_pairs
            .entry(pair_id)
            .or_insert_with(|| SortedOracleList::new(expiry));

        // Check for extreme deviation from current median
        if let Some(current_median) = list.median(timestamp) {
            if !current_median.is_zero() {
                let deviation = if value > current_median {
                    (value - current_median) * U256::from(BPS_DENOMINATOR) / current_median
                } else {
                    (current_median - value) * U256::from(BPS_DENOMINATOR) / current_median
                };
                if deviation > U256::from(BREAKER_THRESHOLD_BPS) {
                    self.frozen_pairs.insert(pair_id, true);
                    return Err(MagnusPrecompileError::Oracle(
                        "rate deviation exceeds threshold, circuit breaker tripped".into(),
                    ));
                }
            }
        }

        list.report(reporter, value, timestamp);
        Ok(())
    }

    /// Get the median rate for a pair.
    pub fn get_rate(
        &mut self,
        base_token: Address,
        quote_token: Address,
        timestamp: u64,
    ) -> Result<U256> {
        let pair_id = rate_pair_id(base_token, quote_token);
        let list = self
            .rate_pairs
            .get_mut(&pair_id)
            .ok_or_else(|| MagnusPrecompileError::Oracle("no reports for rate pair".into()))?;

        list.median(timestamp)
            .ok_or_else(|| MagnusPrecompileError::Oracle("no valid reports (all expired)".into()))
    }

    /// Get the number of valid reports for a pair.
    pub fn num_reports(
        &mut self,
        base_token: Address,
        quote_token: Address,
        timestamp: u64,
    ) -> usize {
        let pair_id = rate_pair_id(base_token, quote_token);
        self.rate_pairs
            .get_mut(&pair_id)
            .map(|list| list.num_valid_reports(timestamp))
            .unwrap_or(0)
    }

    /// Reset the circuit breaker for a pair (governance action).
    pub fn reset_breaker(&mut self, base_token: Address, quote_token: Address) {
        let pair_id = rate_pair_id(base_token, quote_token);
        self.frozen_pairs.remove(&pair_id);
    }

    /// Set custom report expiry for a pair.
    pub fn set_expiry(&mut self, base_token: Address, quote_token: Address, expiry: u64) {
        let pair_id = rate_pair_id(base_token, quote_token);
        self.expiry_overrides.insert(pair_id, expiry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(n: u8) -> Address {
        Address::with_last_byte(n)
    }

    #[test]
    fn add_reporter_and_report() {
        let mut reg = OracleRegistry::new();
        let reporter = addr(1);
        let base = addr(10);
        let quote = addr(20);

        reg.add_reporter(reporter);
        assert!(reg.is_reporter(&reporter));

        let result = reg.report(reporter, base, quote, U256::from(25500), 1000);
        assert!(result.is_ok());

        let rate = reg.get_rate(base, quote, 1000).unwrap();
        assert_eq!(rate, U256::from(25500));
    }

    #[test]
    fn non_reporter_rejected() {
        let mut reg = OracleRegistry::new();
        let result = reg.report(addr(1), addr(10), addr(20), U256::from(100), 1000);
        assert!(result.is_err());
    }

    #[test]
    fn circuit_breaker_trips() {
        let mut reg = OracleRegistry::new();
        let r1 = addr(1);
        let r2 = addr(2);
        let base = addr(10);
        let quote = addr(20);

        reg.add_reporter(r1);
        reg.add_reporter(r2);

        // First report: 1000
        reg.report(r1, base, quote, U256::from(1000), 1000)
            .unwrap();

        // Second report: 1500 (50% deviation > 20% threshold)
        let result = reg.report(r2, base, quote, U256::from(1500), 1001);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("circuit breaker"));
    }

    #[test]
    fn circuit_breaker_reset() {
        let mut reg = OracleRegistry::new();
        let r1 = addr(1);
        let r2 = addr(2);
        let base = addr(10);
        let quote = addr(20);

        reg.add_reporter(r1);
        reg.add_reporter(r2);

        reg.report(r1, base, quote, U256::from(1000), 1000)
            .unwrap();
        let _ = reg.report(r2, base, quote, U256::from(1500), 1001); // trips breaker

        // Reset breaker
        reg.reset_breaker(base, quote);

        // Now reports should work again (but existing reports are still there)
        let result = reg.report(r2, base, quote, U256::from(1100), 1002);
        assert!(result.is_ok());
    }

    #[test]
    fn median_with_multiple_reporters() {
        let mut reg = OracleRegistry::new();
        for i in 1..=5 {
            reg.add_reporter(addr(i));
        }
        let base = addr(10);
        let quote = addr(20);

        // Reports: 100, 102, 105, 103, 101 -> sorted: [100, 101, 102, 103, 105]
        reg.report(addr(1), base, quote, U256::from(100), 1000)
            .unwrap();
        reg.report(addr(2), base, quote, U256::from(102), 1000)
            .unwrap();
        reg.report(addr(3), base, quote, U256::from(105), 1000)
            .unwrap();
        reg.report(addr(4), base, quote, U256::from(103), 1000)
            .unwrap();
        reg.report(addr(5), base, quote, U256::from(101), 1000)
            .unwrap();

        // Median of 5 values = index 2 = 102
        let median = reg.get_rate(base, quote, 1000).unwrap();
        assert_eq!(median, U256::from(102));
    }

    #[test]
    fn reports_expire() {
        let mut reg = OracleRegistry::new();
        reg.add_reporter(addr(1));
        let base = addr(10);
        let quote = addr(20);

        reg.report(addr(1), base, quote, U256::from(100), 1000)
            .unwrap();

        // At t=1360, report expired (360s default)
        let result = reg.get_rate(base, quote, 1360);
        assert!(result.is_err());
    }

    #[test]
    fn custom_expiry() {
        let mut reg = OracleRegistry::new();
        reg.add_reporter(addr(1));
        let base = addr(10);
        let quote = addr(20);

        reg.set_expiry(base, quote, 600); // 10 minute expiry

        reg.report(addr(1), base, quote, U256::from(100), 1000)
            .unwrap();

        // At t=1500, still valid with 600s expiry (age=500 < 600)
        let rate = reg.get_rate(base, quote, 1500);
        assert!(rate.is_ok());
        assert_eq!(rate.unwrap(), U256::from(100));
    }

    #[test]
    fn no_reports_returns_error() {
        let mut reg = OracleRegistry::new();
        let result = reg.get_rate(addr(10), addr(20), 1000);
        assert!(result.is_err());
    }
}
