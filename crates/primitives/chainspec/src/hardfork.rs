//! Magnus-specific hardfork definitions and traits.
//!
//! This module provides the infrastructure for managing hardfork transitions in Magnus.
//!
//! ## Adding a New Hardfork
//!
//! When a new hardfork is needed (e.g., `Vivace`):
//!
//! ### In `hardfork.rs`:
//! 1. Append a `Vivace` variant to `magnus_hardfork!` — automatically:
//!    * defines the enum variant via [`hardfork!`]
//!    * implements trait `MagnusHardforks` by adding `is_vivace()`, `is_vivace_active_at_timestamp()`,
//!      and updating `magnus_hardfork_at()`
//!    * adds tests for each of the `MagnusHardfork` methods
//! 2. Update `From<MagnusHardfork> for SpecId` if the hardfork requires a different Ethereum `SpecId`
//!
//! ### In `spec.rs`:
//! 3. Add `vivace_time: Option<u64>` field to `MagnusGenesisInfo`
//! 4. Add `MagnusHardfork::Vivace => self.vivace_time` arm to `MagnusGenesisInfo::fork_time()`
//!
//! ### In genesis files and generator:
//! 5. Add `"vivaceTime": 0` to `genesis/dev.json`
//! 6. Add `vivace_time: Option<u64>` arg to `xtask/src/genesis_args.rs`
//! 7. Add insertion of `"vivaceTime"` to chain_config.extra_fields

use crate::constants::gas;
use alloy_eips::eip7825::MAX_TX_GAS_LIMIT_OSAKA;
use alloy_evm::revm::primitives::hardfork::SpecId;
use alloy_hardforks::hardfork;

/// Single-source hardfork definition macro. Append a new variant and everything else is generated:
///
/// * Defines the `MagnusHardfork` enum via [`hardfork!`] (including `Display`, `FromStr`,
///   `Hardfork` trait impl, and `VARIANTS` const)
/// * Generates `is_<fork>()` inherent methods on `MagnusHardfork` — returns `true` when
///   `*self >= Self::<Fork>`
/// * Generates the `MagnusHardforks` trait with:
///   - `magnus_fork_activation()` (required — the only method implementors provide)
///   - `magnus_hardfork_at()` — walks `VARIANTS` in reverse to find the latest active fork
///   - `is_<fork>_active_at_timestamp()` — per-fork convenience helpers
///   - `general_gas_limit_at()` — gas limit lookup by timestamp
/// * Generates a `#[cfg(test)] mod tests` with activation, naming, trait, and serde tests
///
/// `Genesis` (first variant) is treated as the baseline and does not get `is_*()` methods.
///  All subsequent variants are considered post-Genesis hardforks.
macro_rules! magnus_hardfork {
    (
        $(#[$enum_meta:meta])*
        MagnusHardfork {
            $(#[$genesis_meta:meta])* Genesis,
            $( $(#[$meta:meta])* $variant:ident ),* $(,)?
        }
    ) => {

        // delegate to alloy's `hardfork!` macro
        hardfork!(
            $(#[$enum_meta])*
            MagnusHardfork {
                $(#[$genesis_meta])* Genesis,
                $( $(#[$meta])* $variant ),*
            }
        );

        impl MagnusHardfork {
            paste::paste! {
                $(
                    #[doc = concat!("Returns true if this hardfork is ", stringify!($variant), " or later.")]
                    pub const fn [<is_ $variant:lower>](&self) -> bool {
                        *self as u64 >= Self::$variant as u64
                    }
                )*
            }
        }

        /// Trait for querying Magnus-specific hardfork activations.
        #[cfg(feature = "reth")]
        pub trait MagnusHardforks: reth_chainspec::EthereumHardforks {
            /// Retrieves activation condition for a Magnus-specific hardfork.
            fn magnus_fork_activation(&self, fork: MagnusHardfork) -> reth_chainspec::ForkCondition;

            /// Retrieves the Magnus hardfork active at a given timestamp.
            fn magnus_hardfork_at(&self, timestamp: u64) -> MagnusHardfork {
                for &fork in MagnusHardfork::VARIANTS.iter().rev() {
                    if self.magnus_fork_activation(fork).active_at_timestamp(timestamp) {
                        return fork;
                    }
                }
                MagnusHardfork::Genesis
            }

            paste::paste! {
                $(
                    #[doc = concat!("Returns true if ", stringify!($variant), " is active at the given timestamp.")]
                    fn [<is_ $variant:lower _active_at_timestamp>](&self, timestamp: u64) -> bool {
                        self.magnus_fork_activation(MagnusHardfork::$variant)
                            .active_at_timestamp(timestamp)
                    }
                )*
            }

            /// Returns the general (non-payment) gas limit for the given timestamp and block.
            /// - T1+: fixed at 30M gas
            /// - Pre-T1: calculated as (gas_limit - shared_gas_limit) / 2
            fn general_gas_limit_at(&self, timestamp: u64, gas_limit: u64, shared_gas_limit: u64) -> u64 {
                self.magnus_hardfork_at(timestamp)
                    .general_gas_limit()
                    .unwrap_or_else(|| (gas_limit - shared_gas_limit) / 2)
            }
        }

        #[cfg(all(test, feature = "reth"))]
        mod tests {
            use super::*;
            use MagnusHardfork::*;
            use reth_chainspec::Hardfork;

            #[test]
            fn test_hardfork_name() {
                assert_eq!(Genesis.name(), "Genesis");
                $(assert_eq!($variant.name(), stringify!($variant));)*
            }

            #[test]
            fn test_hardfork_trait_implementation() {
                for fork in MagnusHardfork::VARIANTS {
                    let _name: &str = Hardfork::name(fork);
                }
            }

            #[test]
            #[cfg(feature = "serde")]
            fn test_magnus_hardfork_serde() {
                for fork in MagnusHardfork::VARIANTS {
                    let json = serde_json::to_string(fork).expect("serialize");
                    let deserialized: MagnusHardfork = serde_json::from_str(&json).expect("deserialize");
                    assert_eq!(deserialized, *fork);
                }
            }

            paste::paste! {
                $(
                    #[test]
                    fn [<test_is_ $variant:lower>]() {
                        let idx = MagnusHardfork::VARIANTS.iter().position(|v| *v == $variant)
                            .expect(concat!(stringify!($variant), " missing from VARIANTS"));
                        for (i, fork) in MagnusHardfork::VARIANTS.iter().enumerate() {
                            let active = MagnusHardfork::[<is_ $variant:lower>](fork);
                            if i >= idx {
                                assert!(active, "{fork:?} should satisfy is_{}", stringify!([<$variant:lower>]));
                            } else {
                                assert!(!active, "{fork:?} should not satisfy is_{}", stringify!([<$variant:lower>]));
                            }
                        }
                    }
                )*
            }
        }
    };
}

// -------------------------------------------------------------------------------------
// Magnus hardfork definitions — append new variants here.
// -------------------------------------------------------------------------------------
magnus_hardfork! (
    /// Magnus-specific hardforks for network upgrades.
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    #[derive(Default)]
    MagnusHardfork {
        /// Genesis hardfork
        Genesis,
        #[default]
        /// T0 hardfork (default until T1 activates on mainnet)
        T0,
        /// T1 hardfork - adds expiring nonce transactions
        T1,
        /// T1.A hardfork - removes EIP-7825 per-transaction gas limit
        T1A,
        /// T1.B hardfork
        T1B,
        /// T1.C hardfork
        T1C,
        /// T2 hardfork - adds compound transfer policies ([MIP-1015])
        ///
        /// [MIP-1015]: <https://docs.magnus.xyz/protocol/mips/mip-1015>
        T2,
        /// T3 hardfork
        T3,
        /// T4 hardfork
        T4,
    }
);

impl MagnusHardfork {
    /// Returns the base fee for this hardfork in attodollars.
    ///
    /// Attodollars are the atomic gas accounting units at 10^-18 USD precision. Individual attodollars are not representable onchain (since MIP-20 tokens only have 6 decimals), but the unit is used for gas accounting.
    /// - Pre-T1: 10 billion attodollars per gas
    /// - T1+: 20 billion attodollars per gas (targets ~0.1 cent per MIP-20 transfer)
    ///
    /// Economic conversion: ceil(basefee × gas_used / 10^12) = cost in microdollars (MIP-20 tokens)
    pub const fn base_fee(&self) -> u64 {
        if self.is_t1() {
            return gas::MAGNUS_T1_BASE_FEE;
        }
        gas::MAGNUS_T0_BASE_FEE
    }

    /// Returns the fixed general gas limit for T1+, or None for pre-T1.
    /// - Pre-T1: None
    /// - T1+: 30M gas (fixed)
    pub const fn general_gas_limit(&self) -> Option<u64> {
        if self.is_t1() {
            return Some(gas::MAGNUS_T1_GENERAL_GAS_LIMIT);
        }
        None
    }

    /// Returns the per-transaction gas limit cap.
    /// - Pre-T1A: EIP-7825 Osaka limit (16,777,216 gas)
    /// - T1A+: 30M gas (allows maximum-sized contract deployments under [MIP-1000] state creation)
    ///
    /// [MIP-1000]: <https://docs.magnus.xyz/protocol/mips/mip-1000>
    pub const fn tx_gas_limit_cap(&self) -> Option<u64> {
        if self.is_t1a() {
            return Some(gas::MAGNUS_T1_TX_GAS_LIMIT_CAP);
        }
        Some(MAX_TX_GAS_LIMIT_OSAKA)
    }

    /// Gas cost for using an existing 2D nonce key
    pub const fn gas_existing_nonce_key(&self) -> u64 {
        if self.is_t2() {
            return gas::MAGNUS_T2_EXISTING_NONCE_KEY_GAS;
        }
        gas::MAGNUS_T1_EXISTING_NONCE_KEY_GAS
    }

    /// Gas cost for using a new 2D nonce key
    pub const fn gas_new_nonce_key(&self) -> u64 {
        if self.is_t2() {
            return gas::MAGNUS_T2_NEW_NONCE_KEY_GAS;
        }
        gas::MAGNUS_T1_NEW_NONCE_KEY_GAS
    }

    /// Returns the active hardfork at the given timestamp for the specified chain.
    ///
    /// Returns `None` if the chain ID is not a known Magnus chain.
    pub const fn from_chain_and_timestamp(chain_id: u64, timestamp: u64) -> Option<Self> {
        // Walk variants in reverse to find the latest active fork, mirroring
        // `MagnusHardforks::magnus_hardfork_at` but without needing a chainspec instance.
        let variants = Self::VARIANTS;
        let mut i = variants.len();
        while i > 0 {
            i -= 1;
            let activation = match chain_id {
                5050 => variants[i].mainnet_activation_timestamp(),
                50500 => variants[i].moderato_activation_timestamp(),
                _ => return None,
            };
            if let Some(ts) = activation
                && timestamp >= ts
            {
                return Some(variants[i]);
            }
        }
        Some(Self::Genesis)
    }

    /// Retrieves the activation block for this hardfork on mainnet.
    pub const fn mainnet_activation_block(&self) -> Option<u64> {
        use crate::constants::mainnet::*;
        match self {
            Self::Genesis => Some(MAINNET_GENESIS_BLOCK),
            Self::T0 => Some(MAINNET_T0_BLOCK),
            Self::T1 => Some(MAINNET_T1_BLOCK),
            Self::T1A => Some(MAINNET_T1A_BLOCK),
            Self::T1B => Some(MAINNET_T1B_BLOCK),
            Self::T1C => Some(MAINNET_T1C_BLOCK),
            Self::T2 => Some(MAINNET_T2_BLOCK),
            Self::T3 => None, // not yet known
            Self::T4 => None,
        }
    }

    /// Retrieves the activation timestamp for this hardfork on mainnet.
    pub const fn mainnet_activation_timestamp(&self) -> Option<u64> {
        use crate::constants::mainnet::*;
        match self {
            Self::Genesis => Some(MAINNET_GENESIS_TIMESTAMP),
            Self::T0 => Some(MAINNET_T0_TIMESTAMP),
            Self::T1 => Some(MAINNET_T1_TIMESTAMP),
            Self::T1A => Some(MAINNET_T1A_TIMESTAMP),
            Self::T1B => Some(MAINNET_T1B_TIMESTAMP),
            Self::T1C => Some(MAINNET_T1C_TIMESTAMP),
            Self::T2 => Some(MAINNET_T2_TIMESTAMP),
            Self::T3 => Some(MAINNET_T3_TIMESTAMP),
            Self::T4 => None,
        }
    }

    /// Retrieves the activation block for this hardfork on moderato testnet.
    pub const fn moderato_activation_block(&self) -> Option<u64> {
        use crate::constants::moderato::*;
        match self {
            Self::Genesis => Some(MODERATO_GENESIS_BLOCK),
            Self::T0 => Some(MODERATO_T0_BLOCK),
            Self::T1 => Some(MODERATO_T1_BLOCK),
            Self::T1A => Some(MODERATO_T1A_BLOCK),
            Self::T1B => Some(MODERATO_T1B_BLOCK),
            Self::T1C => Some(MODERATO_T1C_BLOCK),
            Self::T2 => Some(MODERATO_T2_BLOCK),
            Self::T3 => None, // not yet known
            Self::T4 => None,
        }
    }

    /// Retrieves the activation timestamp for this hardfork on moderato testnet.
    pub const fn moderato_activation_timestamp(&self) -> Option<u64> {
        use crate::constants::moderato::*;
        match self {
            Self::Genesis => Some(MODERATO_GENESIS_TIMESTAMP),
            Self::T0 => Some(MODERATO_T0_TIMESTAMP),
            Self::T1 => Some(MODERATO_T1_TIMESTAMP),
            Self::T1A => Some(MODERATO_T1A_TIMESTAMP),
            Self::T1B => Some(MODERATO_T1B_TIMESTAMP),
            Self::T1C => Some(MODERATO_T1C_TIMESTAMP),
            Self::T2 => Some(MODERATO_T2_TIMESTAMP),
            Self::T3 => Some(MODERATO_T3_TIMESTAMP),
            Self::T4 => None,
        }
    }
}

impl From<MagnusHardfork> for SpecId {
    fn from(_value: MagnusHardfork) -> Self {
        Self::OSAKA
    }
}

impl From<&MagnusHardfork> for SpecId {
    fn from(value: &MagnusHardfork) -> Self {
        Self::from(*value)
    }
}

impl From<SpecId> for MagnusHardfork {
    fn from(_spec: SpecId) -> Self {
        // All Magnus hardforks map to SpecId::OSAKA, so we cannot derive the hardfork from SpecId.
        // Default to the default hardfork when converting from SpecId.
        // The actual hardfork should be passed explicitly where needed.
        Self::default()
    }
}
