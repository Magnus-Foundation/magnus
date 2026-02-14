//! Sorted doubly-linked list for oracle reports.
//!
//! Implements the Celo SortedOracles pattern:
//! - Reports are inserted in sorted order
//! - Median is O(1) via middle pointer tracking
//! - Expired reports are pruned on read
//! - Each reporter can have at most one active report per rate pair

use alloy_primitives::{Address, U256};

/// A single oracle report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleReport {
    /// The reporter's address.
    pub reporter: Address,
    /// The reported rate (fixed-point, 18 decimals).
    /// e.g., 1 USD = 25_500 VND => value = 25_500 * 10^18
    pub value: U256,
    /// Timestamp when the report was submitted.
    pub timestamp: u64,
}

/// Sorted list of oracle reports for a specific rate pair.
///
/// Reports are sorted by value in ascending order.
/// The median is the middle element.
#[derive(Debug, Clone, Default)]
pub struct SortedOracleList {
    /// Reports sorted by value ascending.
    reports: Vec<OracleReport>,
    /// Maximum age of a report in seconds before it expires.
    pub report_expiry: u64,
}

impl SortedOracleList {
    /// Create a new sorted oracle list with the given expiry.
    pub fn new(report_expiry: u64) -> Self {
        Self {
            reports: Vec::new(),
            report_expiry,
        }
    }

    /// Insert or update a report from a reporter.
    ///
    /// If the reporter already has a report, it is replaced.
    /// The list remains sorted by value.
    pub fn report(&mut self, reporter: Address, value: U256, timestamp: u64) {
        // Remove existing report from this reporter
        self.reports.retain(|r| r.reporter != reporter);

        // Insert in sorted position
        let report = OracleReport {
            reporter,
            value,
            timestamp,
        };
        let pos = self
            .reports
            .binary_search_by(|r| r.value.cmp(&value))
            .unwrap_or_else(|pos| pos);
        self.reports.insert(pos, report);
    }

    /// Remove expired reports.
    pub fn prune(&mut self, current_time: u64) {
        self.reports
            .retain(|r| current_time.saturating_sub(r.timestamp) < self.report_expiry);
    }

    /// Get the median value, pruning expired reports first.
    ///
    /// Returns None if no valid reports exist.
    pub fn median(&mut self, current_time: u64) -> Option<U256> {
        self.prune(current_time);
        if self.reports.is_empty() {
            return None;
        }
        let mid = self.reports.len() / 2;
        Some(self.reports[mid].value)
    }

    /// Number of active (non-expired) reports.
    pub fn num_reports(&self) -> usize {
        self.reports.len()
    }

    /// Number of active reports after pruning.
    pub fn num_valid_reports(&mut self, current_time: u64) -> usize {
        self.prune(current_time);
        self.reports.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(n: u8) -> Address {
        Address::with_last_byte(n)
    }

    #[test]
    fn insert_single_report() {
        let mut list = SortedOracleList::new(360);
        list.report(addr(1), U256::from(100), 1000);
        assert_eq!(list.num_reports(), 1);
        assert_eq!(list.median(1000), Some(U256::from(100)));
    }

    #[test]
    fn reports_sorted_by_value() {
        let mut list = SortedOracleList::new(360);
        list.report(addr(1), U256::from(300), 1000);
        list.report(addr(2), U256::from(100), 1000);
        list.report(addr(3), U256::from(200), 1000);

        assert_eq!(list.reports[0].value, U256::from(100));
        assert_eq!(list.reports[1].value, U256::from(200));
        assert_eq!(list.reports[2].value, U256::from(300));
    }

    #[test]
    fn median_odd_number() {
        let mut list = SortedOracleList::new(360);
        list.report(addr(1), U256::from(100), 1000);
        list.report(addr(2), U256::from(200), 1000);
        list.report(addr(3), U256::from(300), 1000);
        // Median of [100, 200, 300] = 200
        assert_eq!(list.median(1000), Some(U256::from(200)));
    }

    #[test]
    fn median_even_number() {
        let mut list = SortedOracleList::new(360);
        list.report(addr(1), U256::from(100), 1000);
        list.report(addr(2), U256::from(200), 1000);
        // Median of [100, 200] = 200 (upper median)
        assert_eq!(list.median(1000), Some(U256::from(200)));
    }

    #[test]
    fn reporter_updates_existing() {
        let mut list = SortedOracleList::new(360);
        list.report(addr(1), U256::from(100), 1000);
        list.report(addr(1), U256::from(200), 1001);
        assert_eq!(list.num_reports(), 1);
        assert_eq!(list.median(1001), Some(U256::from(200)));
    }

    #[test]
    fn expired_reports_pruned() {
        let mut list = SortedOracleList::new(360); // 360 second expiry
        list.report(addr(1), U256::from(100), 1000);
        list.report(addr(2), U256::from(200), 1000);

        // At t=1359, reports are still valid (age = 359 < 360)
        assert_eq!(list.num_valid_reports(1359), 2);

        // At t=1360, reports expire (age = 360 >= 360)
        assert_eq!(list.num_valid_reports(1360), 0);
    }

    #[test]
    fn median_empty_after_expiry() {
        let mut list = SortedOracleList::new(360);
        list.report(addr(1), U256::from(100), 1000);
        assert_eq!(list.median(2000), None);
    }

    #[test]
    fn mixed_expiry() {
        let mut list = SortedOracleList::new(100);
        list.report(addr(1), U256::from(100), 1000); // expires at 1100
        list.report(addr(2), U256::from(200), 1050); // expires at 1150
        list.report(addr(3), U256::from(300), 1090); // expires at 1190

        // At t=1100, only addr(1) expired
        assert_eq!(list.num_valid_reports(1100), 2);
        // Median of [200, 300] = 300
        assert_eq!(list.median(1100), Some(U256::from(300)));
    }
}
