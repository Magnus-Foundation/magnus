//! Per-currency allowlist of governance-approved MIP-20 issuers.
//!
//! See `transfer-station/multi-currency-fees-design.md` §4.

pub mod dispatch;

pub use magnus_contracts::precompiles::{
    IMIP20IssuerRegistry, MIP20IssuerRegistryError, MIP20IssuerRegistryEvent,
};
use magnus_precompiles_macros::contract;

use crate::{
    MIP20_ISSUER_REGISTRY_ADDRESS,
    error::Result,
    mip_fee_manager::{MipFeeManager, currency_registry::currency_key},
    storage::{Handler, Mapping},
};
use alloy::primitives::{Address, B256};

#[contract(addr = MIP20_ISSUER_REGISTRY_ADDRESS)]
pub struct MIP20IssuerRegistry {
    /// `(currency, issuer) -> approved`. Key for outer mapping is `keccak256(currency)`.
    approved_issuers: Mapping<B256, Mapping<Address, bool>>,
    /// Enumeration of approved issuers per currency.
    approved_issuer_list: Mapping<B256, Vec<Address>>,
}

impl MIP20IssuerRegistry {
    pub fn initialize(&mut self) -> Result<()> {
        self.__initialize()
    }

    pub fn is_approved_issuer(&self, currency: &str, issuer: Address) -> Result<bool> {
        self.approved_issuers[currency_key(currency)][issuer].read()
    }

    pub fn get_approved_issuers(&self, currency: &str) -> Result<Vec<Address>> {
        self.approved_issuer_list[currency_key(currency)].read()
    }

    /// Approves `issuer` to deploy MIP-20 tokens of `currency`.
    /// Currency must already be registered in the FeeManager. Caller must be governance admin.
    pub fn add_approved_issuer(
        &mut self,
        sender: Address,
        currency: &str,
        issuer: Address,
    ) -> Result<()> {
        self.assert_governance(sender)?;

        let fm = MipFeeManager::new();
        if !fm.get_currency_config(currency)?.registered {
            return Err(MIP20IssuerRegistryError::currency_not_registered(currency.into()).into());
        }

        let key = currency_key(currency);
        if self.approved_issuers[key][issuer].read()? {
            return Err(MIP20IssuerRegistryError::issuer_already_approved(
                issuer,
                currency.into(),
            )
            .into());
        }

        let mut list = self.approved_issuer_list[key].read()?;
        list.push(issuer);
        self.approved_issuer_list[key].write(list)?;
        self.approved_issuers[key][issuer].write(true)?;

        self.emit_event(MIP20IssuerRegistryEvent::IssuerApproved(
            IMIP20IssuerRegistry::IssuerApproved {
                currency: currency.into(),
                issuer,
            },
        ))?;
        Ok(())
    }

    /// Revokes `issuer`'s approval for `currency`. Caller must be governance admin.
    pub fn remove_approved_issuer(
        &mut self,
        sender: Address,
        currency: &str,
        issuer: Address,
    ) -> Result<()> {
        self.assert_governance(sender)?;

        let key = currency_key(currency);
        if !self.approved_issuers[key][issuer].read()? {
            return Err(MIP20IssuerRegistryError::issuer_not_in_allowlist(
                issuer,
                currency.into(),
            )
            .into());
        }

        let mut list = self.approved_issuer_list[key].read()?;
        let pos = list.iter().position(|a| *a == issuer).ok_or_else(|| {
            crate::error::MagnusPrecompileError::Fatal(
                "approved_issuers flag set but issuer missing from list".into(),
            )
        })?;
        list.swap_remove(pos);
        self.approved_issuer_list[key].write(list)?;
        self.approved_issuers[key][issuer].write(false)?;

        self.emit_event(MIP20IssuerRegistryEvent::IssuerRevoked(
            IMIP20IssuerRegistry::IssuerRevoked {
                currency: currency.into(),
                issuer,
            },
        ))?;
        Ok(())
    }

    fn assert_governance(&self, sender: Address) -> Result<()> {
        let admin = MipFeeManager::new().governance_admin()?;
        if sender != admin {
            return Err(MIP20IssuerRegistryError::only_governance_admin(sender).into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{ContractStorage, StorageCtx, hashmap::HashMapStorageProvider};

    fn fee_manager_with_admin(admin: Address) -> Result<MipFeeManager> {
        let mut fm = MipFeeManager::new();
        fm.initialize()?;
        fm.set_governance_admin(Address::ZERO, admin)?;
        fm.add_currency(admin, "USD", 0)?;
        fm.enable_currency(admin, "USD", 0)?;
        Ok(fm)
    }

    fn registry_initialized() -> Result<MIP20IssuerRegistry> {
        let mut r = MIP20IssuerRegistry::new();
        r.initialize()?;
        Ok(r)
    }

    #[test]
    fn unconfigured_returns_false_and_empty() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let registry = registry_initialized()?;
            assert!(!registry.is_approved_issuer("USD", Address::random())?);
            assert!(registry.get_approved_issuers("USD")?.is_empty());
            Ok(())
        })
    }

    #[test]
    fn add_approved_issuer_happy_path() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let issuer = Address::random();
        StorageCtx::enter(&mut storage, || {
            let _fm = fee_manager_with_admin(admin)?;
            let mut registry = registry_initialized()?;
            registry.clear_emitted_events();

            registry.add_approved_issuer(admin, "USD", issuer)?;

            assert!(registry.is_approved_issuer("USD", issuer)?);
            assert_eq!(registry.get_approved_issuers("USD")?, vec![issuer]);

            registry.assert_emitted_events(vec![MIP20IssuerRegistryEvent::IssuerApproved(
                IMIP20IssuerRegistry::IssuerApproved {
                    currency: "USD".into(),
                    issuer,
                },
            )]);
            Ok(())
        })
    }

    #[test]
    fn add_approved_issuer_rejects_non_admin() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let attacker = Address::random();
        StorageCtx::enter(&mut storage, || {
            let _fm = fee_manager_with_admin(admin)?;
            let mut registry = registry_initialized()?;

            let err = registry
                .add_approved_issuer(attacker, "USD", Address::random())
                .unwrap_err();
            assert_eq!(
                err,
                crate::error::MagnusPrecompileError::MIP20IssuerRegistry(
                    MIP20IssuerRegistryError::only_governance_admin(attacker),
                )
            );
            Ok(())
        })
    }

    #[test]
    fn add_approved_issuer_rejects_unregistered_currency() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = MipFeeManager::new();
            fm.initialize()?;
            fm.set_governance_admin(Address::ZERO, admin)?;
            // VND not added.
            let mut registry = registry_initialized()?;

            let err = registry
                .add_approved_issuer(admin, "VND", Address::random())
                .unwrap_err();
            assert!(matches!(
                err,
                crate::error::MagnusPrecompileError::MIP20IssuerRegistry(
                    MIP20IssuerRegistryError::CurrencyNotRegistered(_)
                )
            ));
            Ok(())
        })
    }

    #[test]
    fn add_approved_issuer_rejects_double_add() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let issuer = Address::random();
        StorageCtx::enter(&mut storage, || {
            let _fm = fee_manager_with_admin(admin)?;
            let mut registry = registry_initialized()?;
            registry.add_approved_issuer(admin, "USD", issuer)?;

            let err = registry
                .add_approved_issuer(admin, "USD", issuer)
                .unwrap_err();
            assert!(matches!(
                err,
                crate::error::MagnusPrecompileError::MIP20IssuerRegistry(
                    MIP20IssuerRegistryError::IssuerAlreadyApproved(_)
                )
            ));
            Ok(())
        })
    }

    #[test]
    fn remove_approved_issuer_happy_path() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let issuer = Address::random();
        StorageCtx::enter(&mut storage, || {
            let _fm = fee_manager_with_admin(admin)?;
            let mut registry = registry_initialized()?;
            registry.add_approved_issuer(admin, "USD", issuer)?;
            registry.clear_emitted_events();

            registry.remove_approved_issuer(admin, "USD", issuer)?;

            assert!(!registry.is_approved_issuer("USD", issuer)?);
            assert!(registry.get_approved_issuers("USD")?.is_empty());

            registry.assert_emitted_events(vec![MIP20IssuerRegistryEvent::IssuerRevoked(
                IMIP20IssuerRegistry::IssuerRevoked {
                    currency: "USD".into(),
                    issuer,
                },
            )]);
            Ok(())
        })
    }

    #[test]
    fn remove_approved_issuer_rejects_unapproved() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        StorageCtx::enter(&mut storage, || {
            let _fm = fee_manager_with_admin(admin)?;
            let mut registry = registry_initialized()?;
            let err = registry
                .remove_approved_issuer(admin, "USD", Address::random())
                .unwrap_err();
            assert!(matches!(
                err,
                crate::error::MagnusPrecompileError::MIP20IssuerRegistry(
                    MIP20IssuerRegistryError::IssuerNotInAllowlist(_)
                )
            ));
            Ok(())
        })
    }

    #[test]
    fn per_currency_scope_is_independent() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let circle = Address::repeat_byte(0xC1);
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_with_admin(admin)?;
            fm.add_currency(admin, "VND", 0)?;
            fm.enable_currency(admin, "VND", 0)?;
            let mut registry = registry_initialized()?;

            registry.add_approved_issuer(admin, "USD", circle)?;
            assert!(registry.is_approved_issuer("USD", circle)?);
            // Approval for USD does NOT carry over to VND.
            assert!(!registry.is_approved_issuer("VND", circle)?);
            Ok(())
        })
    }

    #[test]
    fn multiple_issuers_per_currency() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let circle = Address::repeat_byte(0xC1);
        let tether = Address::repeat_byte(0xC2);
        let paypal = Address::repeat_byte(0xC3);
        StorageCtx::enter(&mut storage, || {
            let _fm = fee_manager_with_admin(admin)?;
            let mut registry = registry_initialized()?;

            registry.add_approved_issuer(admin, "USD", circle)?;
            registry.add_approved_issuer(admin, "USD", tether)?;
            registry.add_approved_issuer(admin, "USD", paypal)?;

            assert!(registry.is_approved_issuer("USD", circle)?);
            assert!(registry.is_approved_issuer("USD", tether)?);
            assert!(registry.is_approved_issuer("USD", paypal)?);
            assert_eq!(registry.get_approved_issuers("USD")?.len(), 3);

            // Remove the middle one; others stay.
            registry.remove_approved_issuer(admin, "USD", tether)?;
            assert!(registry.is_approved_issuer("USD", circle)?);
            assert!(!registry.is_approved_issuer("USD", tether)?);
            assert!(registry.is_approved_issuer("USD", paypal)?);
            assert_eq!(registry.get_approved_issuers("USD")?.len(), 2);
            Ok(())
        })
    }

    #[test]
    fn initialize_is_idempotent_across_handles() -> eyre::Result<()> {
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
