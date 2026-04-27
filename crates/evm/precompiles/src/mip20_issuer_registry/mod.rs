//! Per-currency allowlist of governance-approved MIP-20 issuers.
//!
//! See `transfer-station/multi-currency-fees-design.md` §4. Stub implementation;
//! governance verification + approval logic to be added later.

pub mod dispatch;

pub use magnus_contracts::precompiles::{
    IMIP20IssuerRegistry, MIP20IssuerRegistryError, MIP20IssuerRegistryEvent,
};
use magnus_precompiles_macros::contract;

use crate::{MIP20_ISSUER_REGISTRY_ADDRESS, error::Result};
use alloy::primitives::Address;

/// Per-currency issuer allowlist precompile.
///
/// Storage layout (to be populated): `approved_issuers: Mapping<String, Mapping<Address, bool>>`
/// and `approved_issuer_list: Mapping<String, Vec<Address>>` for enumeration.
#[contract(addr = MIP20_ISSUER_REGISTRY_ADDRESS)]
pub struct MIP20IssuerRegistry {}

impl MIP20IssuerRegistry {
    pub fn initialize(&mut self) -> Result<()> {
        self.__initialize()
    }

    /// Stub: always returns `false`.
    pub fn is_approved_issuer(&self, _currency: &str, _issuer: Address) -> Result<bool> {
        Ok(false)
    }

    /// Stub: always returns an empty vector.
    pub fn get_approved_issuers(&self, _currency: &str) -> Result<Vec<Address>> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{ContractStorage, StorageCtx, hashmap::HashMapStorageProvider};

    #[test]
    fn stub_is_approved_issuer_always_returns_false() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut registry = MIP20IssuerRegistry::new();
            registry.initialize()?;
            for currency in &["USD", "VND", "EUR", "GBP", "JPY", "UNKNOWN"] {
                for _ in 0..3 {
                    let issuer = Address::random();
                    assert!(!registry.is_approved_issuer(currency, issuer)?);
                }
            }
            Ok(())
        })
    }

    #[test]
    fn stub_get_approved_issuers_always_empty() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut registry = MIP20IssuerRegistry::new();
            registry.initialize()?;
            for currency in &["USD", "VND", "", "INVALID_BUT_STILL_OK_FOR_STUB"] {
                assert!(registry.get_approved_issuers(currency)?.is_empty());
            }
            Ok(())
        })
    }

    #[test]
    fn stub_initialize_is_idempotent_across_handles() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut r1 = MIP20IssuerRegistry::new();
            assert!(!r1.is_initialized()?);
            r1.initialize()?;
            assert!(r1.is_initialized()?);
            let r2 = MIP20IssuerRegistry::new();
            assert!(r2.is_initialized()?);
            Ok(())
        })
    }
}
