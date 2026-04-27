//! Off-boarding escrow records (spec §11.2).

use magnus_precompiles_macros::Storable;

/// Per-validator off-board record. `claimed` exists per-(validator, token) at the
/// `escrowed_fees` map; this record tracks the validator-level deadline window.
#[derive(Clone, Debug, Default, PartialEq, Eq, Storable)]
pub struct ClaimRecord {
    pub offboarded: bool,
    pub offboarded_at: u64,
    pub claim_deadline: u64,
}

impl ClaimRecord {
    pub const fn new(now_ts: u64, claim_window_secs: u64) -> Self {
        Self {
            offboarded: true,
            offboarded_at: now_ts,
            claim_deadline: now_ts.saturating_add(claim_window_secs),
        }
    }

    pub const fn within_claim_window(&self, now_ts: u64) -> bool {
        self.offboarded && now_ts <= self.claim_deadline
    }

    pub const fn after_claim_window(&self, now_ts: u64) -> bool {
        self.offboarded && now_ts > self.claim_deadline
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_marks_offboarded_and_computes_deadline() {
        let r = ClaimRecord::new(1_000, 365 * 24 * 60 * 60);
        assert!(r.offboarded);
        assert_eq!(r.offboarded_at, 1_000);
        assert_eq!(r.claim_deadline, 1_000 + 365 * 24 * 60 * 60);
    }

    #[test]
    fn window_predicates_split_at_deadline_inclusively() {
        let r = ClaimRecord::new(0, 100);
        assert!(r.within_claim_window(0));
        assert!(r.within_claim_window(100));
        assert!(!r.within_claim_window(101));
        assert!(r.after_claim_window(101));
        assert!(!r.after_claim_window(100));
    }

    #[test]
    fn default_record_is_not_offboarded() {
        let r = ClaimRecord::default();
        assert!(!r.offboarded);
        assert!(!r.within_claim_window(0));
        assert!(!r.after_claim_window(u64::MAX));
    }
}
