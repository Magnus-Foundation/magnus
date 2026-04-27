//! [MIP-20 Issuer Registry] precompile — per-currency allowlist of governance-approved
//! token issuers.
//!
//! See [`transfer-station/multi-currency-fees-design.md`] §4 for the v3.8.2 spec.
//! Introduced in T4 hardfork as part of the multi-currency fees model.
//!
//! **G0 status:** scaffold only — storage layout and dispatch wiring exist; governance
//! verification + actual approval logic land in G4. Calls currently return
//! `MethodNotImplemented` style placeholders so downstream tests can target the precompile
//! address but no real allowlist is enforced yet.
//!
//! [MIP-20 Issuer Registry]: <https://docs.magnus.xyz/protocol/mip20-issuer-registry>

pub mod dispatch;

pub use magnus_contracts::precompiles::{
    IMIP20IssuerRegistry, MIP20IssuerRegistryError, MIP20IssuerRegistryEvent,
};
use magnus_precompiles_macros::contract;

use crate::{MIP20_ISSUER_REGISTRY_ADDRESS, error::Result};
use alloy::primitives::Address;

/// Per-currency issuer allowlist precompile.
///
/// Storage layout (G0 stub — finalized in G4):
/// - `approved_issuers: Mapping<String, Mapping<Address, bool>>` — `(currency, issuer) → approved`
/// - `approved_issuer_list: Mapping<String, Vec<Address>>` — for enumeration
///
/// Both maps are filled in during G4. The struct is empty in G0 so the `#[contract]`
/// macro can generate the storage handlers and the precompile address can resolve.
#[contract(addr = MIP20_ISSUER_REGISTRY_ADDRESS)]
pub struct MIP20IssuerRegistry {}

// Precompile functions
impl MIP20IssuerRegistry {
    /// Initializes the issuer registry precompile.
    pub fn initialize(&mut self) -> Result<()> {
        self.__initialize()
    }

    /// Returns whether `issuer` is approved to deploy MIP-20s of the given `currency`.
    ///
    /// **G0 stub:** always returns `false`. Real allowlist lookup lands in G4.
    pub fn is_approved_issuer(&self, _currency: &str, _issuer: Address) -> Result<bool> {
        Ok(false)
    }

    /// Returns the list of approved issuer addresses for the given `currency`.
    ///
    /// **G0 stub:** always returns an empty vector. Real enumeration lands in G4.
    pub fn get_approved_issuers(&self, _currency: &str) -> Result<Vec<Address>> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{ContractStorage, StorageCtx, hashmap::HashMapStorageProvider};

    /// G0 contract: `is_approved_issuer` always returns `false` until the
    /// real allowlist lookup lands in G4. This test pins that behavior so
    /// future regressions don't accidentally start returning `true` for
    /// uninitialized state.
    #[test]
    fn stub_is_approved_issuer_always_returns_false() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);

        StorageCtx::enter(&mut storage, || {
            let mut registry = MIP20IssuerRegistry::new();
            registry.initialize()?;

            // Random issuers, common currency codes — all must return false.
            for currency in &["USD", "VND", "EUR", "GBP", "JPY", "UNKNOWN"] {
                for _ in 0..3 {
                    let issuer = Address::random();
                    assert!(
                        !registry.is_approved_issuer(currency, issuer)?,
                        "G0 stub must return false for ({}, {})",
                        currency,
                        issuer,
                    );
                }
            }

            Ok(())
        })
    }

    /// G0 contract: `get_approved_issuers` always returns empty vec. Pins
    /// the stub behavior — the real enumeration lands in G4.
    #[test]
    fn stub_get_approved_issuers_always_empty() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);

        StorageCtx::enter(&mut storage, || {
            let mut registry = MIP20IssuerRegistry::new();
            registry.initialize()?;

            for currency in &["USD", "VND", "", "INVALID_BUT_STILL_OK_FOR_STUB"] {
                let result = registry.get_approved_issuers(currency)?;
                assert!(
                    result.is_empty(),
                    "G0 stub must return empty list for currency {:?}, got {:?}",
                    currency,
                    result
                );
            }

            Ok(())
        })
    }

    /// Verifies the `#[contract]` macro generates a usable `new`/`initialize`
    /// pair and that initialization is idempotent across handle creation.
    #[test]
    fn stub_initialize_is_idempotent_across_handles() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);

        StorageCtx::enter(&mut storage, || {
            let mut r1 = MIP20IssuerRegistry::new();
            assert!(!r1.is_initialized()?);
            r1.initialize()?;
            assert!(r1.is_initialized()?);

            // Fresh handle still observes initialized state from shared storage.
            let r2 = MIP20IssuerRegistry::new();
            assert!(r2.is_initialized()?);

            Ok(())
        })
    }
}
