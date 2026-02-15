//! Magnus-specific hardfork definitions and traits.
//!
//! This module provides the infrastructure for managing hardfork transitions in Magnus.
//!
//! ## Adding a New Hardfork
//!
//! When a new hardfork is needed (e.g., `Vivace`):
//!
//! ### In `hardfork.rs`:
//! 1. Add a new variant to `MagnusHardfork` enum
//! 2. Add `is_vivace()` method to `MagnusHardfork` impl
//! 3. Add `is_vivace_active_at_timestamp()` to `MagnusHardforks` trait
//! 4. Update `magnus_hardfork_at()` to check for the new hardfork first (latest hardfork is checked first)
//! 5. Add `MagnusHardfork::Vivace => Self::OSAKA` (or appropriate SpecId) in `From<MagnusHardfork> for SpecId`
//! 6. Update `From<SpecId> for MagnusHardfork` to check for the new hardfork first
//! 7. Add test `test_is_vivace` and update existing `is_*` tests to include the new variant
//!
//! ### In `spec.rs`:
//! 8. Add `vivace_time: Option<u64>` field to `MagnusGenesisInfo`
//! 9. Extract `vivace_time` in `MagnusChainSpec::from_genesis`
//! 10. Add `(MagnusHardfork::Vivace, vivace_time)` to `magnus_forks` vec
//! 11. Update tests to include `"vivaceTime": <timestamp>` in genesis JSON
//!
//! ### In genesis files and generator:
//! 12. Add `"vivaceTime": 0` to `genesis/dev.json`
//! 13. Add `vivace_time: Option<u64>` arg to `xtask/src/genesis_args.rs`
//! 14. Add insertion of `"vivaceTime"` to chain_config.extra_fields
//!
//! ## Current State
//!
//! The `Genesis` variant is a placeholder representing the pre-hardfork baseline.

use alloy_evm::revm::primitives::hardfork::SpecId;
use alloy_hardforks::hardfork;
use reth_chainspec::{EthereumHardforks, ForkCondition};

hardfork!(
    /// Magnus-specific hardforks for network upgrades.
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    #[derive(Default)]
    MagnusHardfork {
        /// Genesis hardfork
        Genesis,
        #[default]
        /// T0 hardfork (default)
        T0,
    }
);

impl MagnusHardfork {
    /// Returns true if this hardfork is T0 or later.
    pub fn is_t0(&self) -> bool {
        matches!(self, Self::T0)
    }
}

/// Trait for querying Magnus-specific hardfork activations.
pub trait MagnusHardforks: EthereumHardforks {
    /// Retrieves activation condition for a Magnus-specific hardfork
    fn magnus_fork_activation(&self, fork: MagnusHardfork) -> ForkCondition;

    /// Retrieves the Magnus hardfork active at a given timestamp.
    fn magnus_hardfork_at(&self, timestamp: u64) -> MagnusHardfork {
        if self.is_t0_active_at_timestamp(timestamp) {
            return MagnusHardfork::T0;
        }
        MagnusHardfork::Genesis
    }

    /// Returns true if T0 is active at the given timestamp.
    fn is_t0_active_at_timestamp(&self, timestamp: u64) -> bool {
        self.magnus_fork_activation(MagnusHardfork::T0)
            .active_at_timestamp(timestamp)
    }
}

impl From<MagnusHardfork> for SpecId {
    fn from(_value: MagnusHardfork) -> Self {
        Self::OSAKA
    }
}

impl From<SpecId> for MagnusHardfork {
    fn from(spec: SpecId) -> Self {
        if spec.is_enabled_in(SpecId::from(Self::T0)) {
            Self::T0
        } else {
            Self::Genesis
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reth_chainspec::Hardfork;

    #[test]
    fn test_genesis_hardfork_name() {
        let fork = MagnusHardfork::Genesis;
        assert_eq!(fork.name(), "Genesis");
    }

    #[test]
    fn test_t0_hardfork_name() {
        let fork = MagnusHardfork::T0;
        assert_eq!(fork.name(), "T0");
    }

    #[test]
    fn test_is_t0() {
        assert!(!MagnusHardfork::Genesis.is_t0());
        assert!(MagnusHardfork::T0.is_t0());
    }

    #[test]
    fn test_hardfork_trait_implementation() {
        let fork = MagnusHardfork::Genesis;
        // Should implement Hardfork trait
        let _name: &str = Hardfork::name(&fork);
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_magnus_hardfork_serde() {
        let fork = MagnusHardfork::Genesis;

        // Serialize to JSON
        let json = serde_json::to_string(&fork).unwrap();
        assert_eq!(json, "\"Genesis\"");

        // Deserialize from JSON
        let deserialized: MagnusHardfork = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, fork);
    }
}
