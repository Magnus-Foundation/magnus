pub mod dispatch;

pub use magnus_contracts::precompiles::{IKYCRegistry, KYCRegistryError, KYCRegistryEvent};
use magnus_precompile_macros::{Storable, contract};

use crate::{
    KYC_REGISTRY_ADDRESS,
    error::{MagnusPrecompileError, Result},
    storage::{Handler, Mapping},
};
use alloy::primitives::{Address, U256};

#[derive(Debug, Clone, Default, Storable)]
pub struct KYCRecord {
    pub level: u8,        // 0=none, 1=basic, 2=enhanced, 3=institutional
    pub expiry: u64,
    pub verifier: Address,
    pub jurisdiction: u8, // 0=global, 1=VN, 2=US, 3=EU, etc.
}

#[contract(addr = KYC_REGISTRY_ADDRESS)]
pub struct KYCRegistry {
    owner: Address,
    verifiers: Mapping<Address, bool>,
    kyc_data: Mapping<Address, KYCRecord>,
}

impl KYCRegistry {
    pub fn initialize(&mut self, init_owner: Address) -> Result<()> {
        self.__initialize()?;
        self.owner.write(init_owner)?;
        Ok(())
    }

    // --- View functions ---

    pub fn owner(&self) -> Result<Address> {
        self.owner.read()
    }

    pub fn is_verified(&self, call: IKYCRegistry::isVerifiedCall) -> Result<bool> {
        let record = self.kyc_data[call.account].read()?;
        if record.level == 0 {
            return Ok(false);
        }
        let now: u64 = self.storage.timestamp().try_into().unwrap_or(u64::MAX);
        Ok(record.expiry > now)
    }

    pub fn get_kyc_level(&self, call: IKYCRegistry::getKYCLevelCall) -> Result<u8> {
        let record = self.kyc_data[call.account].read()?;
        Ok(record.level)
    }

    pub fn get_kyc_record(
        &self,
        call: IKYCRegistry::getKYCRecordCall,
    ) -> Result<IKYCRegistry::getKYCRecordReturn> {
        let record = self.kyc_data[call.account].read()?;
        Ok(IKYCRegistry::getKYCRecordReturn {
            level: record.level,
            expiry: record.expiry,
            verifier: record.verifier,
            jurisdiction: record.jurisdiction,
        })
    }

    pub fn is_verifier(&self, call: IKYCRegistry::isVerifierCall) -> Result<bool> {
        self.verifiers[call.verifier].read()
    }

    // --- Verifier functions ---

    pub fn set_verified(
        &mut self,
        msg_sender: Address,
        call: IKYCRegistry::setVerifiedCall,
    ) -> Result<()> {
        self.require_verifier(msg_sender)?;

        if call.level == 0 {
            return Err(KYCRegistryError::invalid_level().into());
        }

        let now: u64 = self.storage.timestamp().try_into().unwrap_or(u64::MAX);
        if call.expiry <= now {
            return Err(KYCRegistryError::expiry_in_past().into());
        }

        let record = KYCRecord {
            level: call.level,
            expiry: call.expiry,
            verifier: msg_sender,
            jurisdiction: call.jurisdiction,
        };
        self.kyc_data[call.account].write(record)?;

        self.emit_event(KYCRegistryEvent::KYCVerified(IKYCRegistry::KYCVerified {
            account: call.account,
            verifier: msg_sender,
            level: call.level,
            expiry: call.expiry,
            jurisdiction: call.jurisdiction,
        }))
    }

    pub fn revoke(
        &mut self,
        msg_sender: Address,
        call: IKYCRegistry::revokeCall,
    ) -> Result<()> {
        self.require_verifier(msg_sender)?;

        let record = self.kyc_data[call.account].read()?;
        if record.level == 0 {
            return Err(KYCRegistryError::kyc_not_found().into());
        }

        self.kyc_data[call.account].write(KYCRecord::default())?;

        self.emit_event(KYCRegistryEvent::KYCRevoked(IKYCRegistry::KYCRevoked {
            account: call.account,
            revoker: msg_sender,
        }))
    }

    pub fn batch_set_verified(
        &mut self,
        msg_sender: Address,
        call: IKYCRegistry::batchSetVerifiedCall,
    ) -> Result<()> {
        self.require_verifier(msg_sender)?;

        if call.level == 0 {
            return Err(KYCRegistryError::invalid_level().into());
        }

        let now: u64 = self.storage.timestamp().try_into().unwrap_or(u64::MAX);
        if call.expiry <= now {
            return Err(KYCRegistryError::expiry_in_past().into());
        }

        for account in &call.accounts {
            let record = KYCRecord {
                level: call.level,
                expiry: call.expiry,
                verifier: msg_sender,
                jurisdiction: call.jurisdiction,
            };
            self.kyc_data[*account].write(record)?;

            self.emit_event(KYCRegistryEvent::KYCVerified(IKYCRegistry::KYCVerified {
                account: *account,
                verifier: msg_sender,
                level: call.level,
                expiry: call.expiry,
                jurisdiction: call.jurisdiction,
            }))?;
        }

        Ok(())
    }

    // --- Owner functions ---

    pub fn add_verifier(
        &mut self,
        msg_sender: Address,
        call: IKYCRegistry::addVerifierCall,
    ) -> Result<()> {
        self.require_owner(msg_sender)?;
        self.verifiers[call.verifier].write(true)?;
        self.emit_event(KYCRegistryEvent::VerifierAdded(
            IKYCRegistry::VerifierAdded {
                verifier: call.verifier,
                addedBy: msg_sender,
            },
        ))
    }

    pub fn remove_verifier(
        &mut self,
        msg_sender: Address,
        call: IKYCRegistry::removeVerifierCall,
    ) -> Result<()> {
        self.require_owner(msg_sender)?;
        self.verifiers[call.verifier].write(false)?;
        self.emit_event(KYCRegistryEvent::VerifierRemoved(
            IKYCRegistry::VerifierRemoved {
                verifier: call.verifier,
                removedBy: msg_sender,
            },
        ))
    }

    pub fn transfer_ownership(
        &mut self,
        msg_sender: Address,
        call: IKYCRegistry::transferOwnershipCall,
    ) -> Result<()> {
        self.require_owner(msg_sender)?;
        self.owner.write(call.newOwner)?;
        self.emit_event(KYCRegistryEvent::OwnershipTransferred(
            IKYCRegistry::OwnershipTransferred {
                previousOwner: msg_sender,
                newOwner: call.newOwner,
            },
        ))
    }

    // --- Internal helpers ---

    fn require_owner(&self, sender: Address) -> Result<()> {
        let owner = self.owner.read()?;
        if sender != owner {
            return Err(KYCRegistryError::unauthorized().into());
        }
        Ok(())
    }

    fn require_verifier(&self, sender: Address) -> Result<()> {
        let is_verifier = self.verifiers[sender].read()?;
        if !is_verifier {
            return Err(KYCRegistryError::unauthorized().into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{ContractStorage, StorageCtx, hashmap::HashMapStorageProvider};

    #[test]
    fn test_set_verified_and_is_verified() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let owner = Address::random();
        let verifier = Address::random();
        let account = Address::random();

        StorageCtx::enter(&mut storage, || {
            let mut registry = KYCRegistry::new();
            registry.initialize(owner)?;
            registry.add_verifier(owner, IKYCRegistry::addVerifierCall { verifier })?;

            // Set timestamp to 1000
            let mut ctx = StorageCtx;
            ctx.set_timestamp(U256::from(1000u64));

            // Verify before setting → false
            assert!(!registry.is_verified(IKYCRegistry::isVerifiedCall { account })?);

            // Set verified with expiry in the future
            registry.set_verified(
                verifier,
                IKYCRegistry::setVerifiedCall {
                    account,
                    level: 2,
                    expiry: 5000,
                    jurisdiction: 1,
                },
            )?;

            // Now should be verified
            assert!(registry.is_verified(IKYCRegistry::isVerifiedCall { account })?);
            assert_eq!(
                registry.get_kyc_level(IKYCRegistry::getKYCLevelCall { account })?,
                2
            );

            let record = registry.get_kyc_record(IKYCRegistry::getKYCRecordCall { account })?;
            assert_eq!(record.level, 2);
            assert_eq!(record.expiry, 5000);
            assert_eq!(record.verifier, verifier);
            assert_eq!(record.jurisdiction, 1);

            Ok(())
        })
    }

    #[test]
    fn test_revoke() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let owner = Address::random();
        let verifier = Address::random();
        let account = Address::random();

        StorageCtx::enter(&mut storage, || {
            let mut registry = KYCRegistry::new();
            registry.initialize(owner)?;
            registry.add_verifier(owner, IKYCRegistry::addVerifierCall { verifier })?;

            let mut ctx = StorageCtx;
            ctx.set_timestamp(U256::from(1000u64));

            registry.set_verified(
                verifier,
                IKYCRegistry::setVerifiedCall {
                    account,
                    level: 1,
                    expiry: 5000,
                    jurisdiction: 0,
                },
            )?;

            assert!(registry.is_verified(IKYCRegistry::isVerifiedCall { account })?);

            // Revoke
            registry.revoke(verifier, IKYCRegistry::revokeCall { account })?;

            // Should no longer be verified
            assert!(!registry.is_verified(IKYCRegistry::isVerifiedCall { account })?);
            assert_eq!(
                registry.get_kyc_level(IKYCRegistry::getKYCLevelCall { account })?,
                0
            );

            Ok(())
        })
    }

    #[test]
    fn test_batch_set_verified() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let owner = Address::random();
        let verifier = Address::random();
        let accounts: Vec<Address> = (0..5).map(|_| Address::random()).collect();

        StorageCtx::enter(&mut storage, || {
            let mut registry = KYCRegistry::new();
            registry.initialize(owner)?;
            registry.add_verifier(owner, IKYCRegistry::addVerifierCall { verifier })?;

            let mut ctx = StorageCtx;
            ctx.set_timestamp(U256::from(1000u64));

            registry.batch_set_verified(
                verifier,
                IKYCRegistry::batchSetVerifiedCall {
                    accounts: accounts.clone(),
                    level: 3,
                    expiry: 10000,
                    jurisdiction: 2,
                },
            )?;

            // All accounts should be verified
            for account in &accounts {
                assert!(registry.is_verified(IKYCRegistry::isVerifiedCall { account: *account })?);
                assert_eq!(
                    registry.get_kyc_level(IKYCRegistry::getKYCLevelCall { account: *account })?,
                    3
                );
            }

            Ok(())
        })
    }

    #[test]
    fn test_expiry_checking() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let owner = Address::random();
        let verifier = Address::random();
        let account = Address::random();

        StorageCtx::enter(&mut storage, || {
            let mut registry = KYCRegistry::new();
            registry.initialize(owner)?;
            registry.add_verifier(owner, IKYCRegistry::addVerifierCall { verifier })?;

            let mut ctx = StorageCtx;
            ctx.set_timestamp(U256::from(1000u64));

            registry.set_verified(
                verifier,
                IKYCRegistry::setVerifiedCall {
                    account,
                    level: 1,
                    expiry: 2000,
                    jurisdiction: 0,
                },
            )?;

            // At t=1000, expiry=2000 → verified
            assert!(registry.is_verified(IKYCRegistry::isVerifiedCall { account })?);

            // Advance time past expiry
            ctx.set_timestamp(U256::from(3000u64));

            // Now expired → not verified (but level still stored)
            assert!(!registry.is_verified(IKYCRegistry::isVerifiedCall { account })?);
            assert_eq!(
                registry.get_kyc_level(IKYCRegistry::getKYCLevelCall { account })?,
                1
            );

            Ok(())
        })
    }

    #[test]
    fn test_unauthorized_access() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let owner = Address::random();
        let non_verifier = Address::random();
        let non_owner = Address::random();
        let account = Address::random();

        StorageCtx::enter(&mut storage, || {
            let mut registry = KYCRegistry::new();
            registry.initialize(owner)?;

            let mut ctx = StorageCtx;
            ctx.set_timestamp(U256::from(1000u64));

            // Non-verifier cannot set_verified
            let result = registry.set_verified(
                non_verifier,
                IKYCRegistry::setVerifiedCall {
                    account,
                    level: 1,
                    expiry: 5000,
                    jurisdiction: 0,
                },
            );
            assert_eq!(
                result,
                Err(MagnusPrecompileError::KYCRegistryError(
                    KYCRegistryError::unauthorized()
                ))
            );

            // Non-owner cannot add verifier
            let result = registry.add_verifier(
                non_owner,
                IKYCRegistry::addVerifierCall {
                    verifier: non_verifier,
                },
            );
            assert_eq!(
                result,
                Err(MagnusPrecompileError::KYCRegistryError(
                    KYCRegistryError::unauthorized()
                ))
            );

            // Non-owner cannot transfer ownership
            let result = registry.transfer_ownership(
                non_owner,
                IKYCRegistry::transferOwnershipCall {
                    newOwner: non_owner,
                },
            );
            assert_eq!(
                result,
                Err(MagnusPrecompileError::KYCRegistryError(
                    KYCRegistryError::unauthorized()
                ))
            );

            Ok(())
        })
    }

    #[test]
    fn test_ownership_transfer() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let owner = Address::random();
        let new_owner = Address::random();

        StorageCtx::enter(&mut storage, || {
            let mut registry = KYCRegistry::new();
            registry.initialize(owner)?;

            registry.transfer_ownership(
                owner,
                IKYCRegistry::transferOwnershipCall { newOwner: new_owner },
            )?;

            assert_eq!(registry.owner()?, new_owner);

            // Old owner can no longer act
            let result = registry.add_verifier(
                owner,
                IKYCRegistry::addVerifierCall {
                    verifier: Address::random(),
                },
            );
            assert_eq!(
                result,
                Err(MagnusPrecompileError::KYCRegistryError(
                    KYCRegistryError::unauthorized()
                ))
            );

            Ok(())
        })
    }
}
