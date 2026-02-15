//! MIP403 Compliance Registry -- transfer authorization checks.
//!
//! Tokens can register compliance rules that are checked on every transfer.
//! This enables regulatory compliance (KYC/AML) at the protocol level.
//!
//! Built-in policies:
//! - Policy 0: always-reject (blocks all transfers)
//! - Policy 1: always-allow (permits all transfers, default)
//! - Policy 2+: user-created whitelist or blacklist policies

use alloy_primitives::{Address, U256};
use crate::{
    addresses,
    error::{MagnusPrecompileError, Result},
    storage::{Mapping, NestedMapping},
};

/// Policy type discriminants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PolicyType {
    /// Only addresses in the set may transfer.
    Whitelist = 0,
    /// All addresses may transfer except those in the set.
    Blacklist = 1,
}

impl TryFrom<u8> for PolicyType {
    type Error = MagnusPrecompileError;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Whitelist),
            1 => Ok(Self::Blacklist),
            _ => Err(MagnusPrecompileError::InvalidInput(
                format!("unknown policy type: {}", value),
            )),
        }
    }
}

/// MIP403 Compliance Registry.
#[derive(Debug)]
pub struct MIP403Registry {
    /// Contract address.
    pub address: Address,
    /// Next policy ID to assign (starts at 2).
    policy_counter: Mapping<Address, U256>,
    /// policy_id -> packed(policy_type, admin): policy_type in low byte, admin in upper 20 bytes.
    policy_data: Mapping<U256, U256>,
    /// policy_id -> account -> authorized (1 = in set, 0 = not in set).
    policy_set: NestedMapping<U256, Address, U256>,
}

/// Storage slot for the global policy counter (uses address(0) as key).
const COUNTER_SLOT: U256 = U256::ZERO;
/// Base slot for policy_data mapping.
const POLICY_DATA_SLOT: U256 = U256::from_limbs([1, 0, 0, 0]);
/// Base slot for policy_set nested mapping.
const POLICY_SET_SLOT: U256 = U256::from_limbs([2, 0, 0, 0]);

/// Pack policy_type and admin into a single U256.
fn pack_policy_data(policy_type: PolicyType, admin: Address) -> U256 {
    let mut bytes = [0u8; 32];
    bytes[0] = policy_type as u8;
    bytes[12..32].copy_from_slice(admin.as_slice());
    U256::from_be_bytes::<32>(bytes)
}

/// Unpack policy_type and admin from a U256.
fn unpack_policy_data(packed: U256) -> Result<(PolicyType, Address)> {
    let bytes = packed.to_be_bytes::<32>();
    let policy_type = PolicyType::try_from(bytes[0])?;
    let admin = Address::from_slice(&bytes[12..32]);
    Ok((policy_type, admin))
}

impl MIP403Registry {
    /// Create a new MIP403 registry instance.
    pub fn new() -> Self {
        Self {
            address: addresses::MIP403_REGISTRY,
            policy_counter: Mapping::new(addresses::MIP403_REGISTRY, COUNTER_SLOT),
            policy_data: Mapping::new(addresses::MIP403_REGISTRY, POLICY_DATA_SLOT),
            policy_set: NestedMapping::new(addresses::MIP403_REGISTRY, POLICY_SET_SLOT),
        }
    }

    /// Read the next policy ID counter. Starts at 2 (0 and 1 are reserved).
    fn next_id(&self) -> u64 {
        let val = self.policy_counter.read(&Address::ZERO);
        let id = val.as_limbs()[0];
        if id < 2 { 2 } else { id }
    }

    /// Increment and return the next policy ID.
    fn increment_counter(&mut self) -> u64 {
        let id = self.next_id();
        self.policy_counter.write(&Address::ZERO, U256::from(id + 1));
        id
    }

    /// Create a new compliance policy.
    ///
    /// Returns the assigned policy ID.
    pub fn create_policy(
        &mut self,
        admin: Address,
        policy_type: PolicyType,
    ) -> Result<u64> {
        let id = self.increment_counter();
        let packed = pack_policy_data(policy_type, admin);
        self.policy_data.write(&U256::from(id), packed);
        Ok(id)
    }

    /// Get the admin and type of a policy.
    pub fn get_policy(&self, policy_id: u64) -> Result<(PolicyType, Address)> {
        if policy_id < 2 {
            return Err(MagnusPrecompileError::InvalidInput(
                "policies 0 and 1 are built-in".into(),
            ));
        }
        let packed = self.policy_data.read(&U256::from(policy_id));
        if packed.is_zero() {
            return Err(MagnusPrecompileError::InvalidInput(
                format!("policy {} does not exist", policy_id),
            ));
        }
        unpack_policy_data(packed)
    }

    /// Update the admin of a policy.
    pub fn set_policy_admin(
        &mut self,
        msg_sender: Address,
        policy_id: u64,
        new_admin: Address,
    ) -> Result<()> {
        let (policy_type, admin) = self.get_policy(policy_id)?;
        if msg_sender != admin {
            return Err(MagnusPrecompileError::Unauthorized(
                "only policy admin can update".into(),
            ));
        }
        let packed = pack_policy_data(policy_type, new_admin);
        self.policy_data.write(&U256::from(policy_id), packed);
        Ok(())
    }

    /// Add or remove an account from a whitelist policy.
    pub fn modify_whitelist(
        &mut self,
        msg_sender: Address,
        policy_id: u64,
        account: Address,
        allowed: bool,
    ) -> Result<()> {
        let (policy_type, admin) = self.get_policy(policy_id)?;
        if msg_sender != admin {
            return Err(MagnusPrecompileError::Unauthorized(
                "only policy admin can modify".into(),
            ));
        }
        if policy_type != PolicyType::Whitelist {
            return Err(MagnusPrecompileError::InvalidInput(
                "not a whitelist policy".into(),
            ));
        }
        let val = if allowed { U256::from(1) } else { U256::ZERO };
        self.policy_set.write(&U256::from(policy_id), &account, val);
        Ok(())
    }

    /// Add or remove an account from a blacklist policy.
    pub fn modify_blacklist(
        &mut self,
        msg_sender: Address,
        policy_id: u64,
        account: Address,
        restricted: bool,
    ) -> Result<()> {
        let (policy_type, admin) = self.get_policy(policy_id)?;
        if msg_sender != admin {
            return Err(MagnusPrecompileError::Unauthorized(
                "only policy admin can modify".into(),
            ));
        }
        if policy_type != PolicyType::Blacklist {
            return Err(MagnusPrecompileError::InvalidInput(
                "not a blacklist policy".into(),
            ));
        }
        let val = if restricted { U256::from(1) } else { U256::ZERO };
        self.policy_set.write(&U256::from(policy_id), &account, val);
        Ok(())
    }

    /// Check if a transfer is authorized under a given policy.
    ///
    /// - Policy 0: always reject
    /// - Policy 1: always allow (default)
    /// - Whitelist policy: authorized only if account is in the set
    /// - Blacklist policy: authorized unless account is in the set
    pub fn is_authorized(&self, policy_id: u64, user: Address) -> Result<bool> {
        // Built-in policies
        if policy_id == 0 {
            return Ok(false);
        }
        if policy_id == 1 {
            return Ok(true);
        }

        let (policy_type, _admin) = self.get_policy(policy_id)?;
        let in_set = !self.policy_set.read(&U256::from(policy_id), &user).is_zero();

        match policy_type {
            PolicyType::Whitelist => Ok(in_set),
            PolicyType::Blacklist => Ok(!in_set),
        }
    }

    /// Check if a transfer between two addresses is authorized.
    ///
    /// Both sender and receiver must be authorized under the policy.
    pub fn is_transfer_authorized(
        &self,
        policy_id: u64,
        from: Address,
        to: Address,
    ) -> Result<bool> {
        Ok(self.is_authorized(policy_id, from)?
            && self.is_authorized(policy_id, to)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{StorageBackend, with_storage};
    use std::collections::HashMap;

    struct MemStorage(std::cell::RefCell<HashMap<(Address, U256), U256>>);

    impl MemStorage {
        fn new() -> Self {
            Self(std::cell::RefCell::new(HashMap::new()))
        }
    }

    impl StorageBackend for MemStorage {
        fn sload(&self, address: Address, slot: U256) -> U256 {
            self.0.borrow().get(&(address, slot)).copied().unwrap_or(U256::ZERO)
        }
        fn sstore(&mut self, address: Address, slot: U256, value: U256) {
            self.0.borrow_mut().insert((address, slot), value);
        }
    }

    fn addr(n: u8) -> Address {
        Address::with_last_byte(n)
    }

    #[test]
    fn builtin_policies() {
        assert_eq!(
            MIP403Registry::new().is_authorized(0, addr(1)).ok(),
            Some(false),
        );
    }

    #[test]
    fn builtin_policy_1_allows() {
        assert_eq!(
            MIP403Registry::new().is_authorized(1, addr(1)).ok(),
            Some(true),
        );
    }

    #[test]
    fn create_whitelist_policy() {
        with_storage(Box::new(MemStorage::new()), || {
            let mut reg = MIP403Registry::new();
            let admin = addr(99);
            let id = reg.create_policy(admin, PolicyType::Whitelist).unwrap();
            assert_eq!(id, 2);

            // Not whitelisted -> denied
            assert!(!reg.is_authorized(id, addr(1)).unwrap());

            // Whitelist addr(1)
            reg.modify_whitelist(admin, id, addr(1), true).unwrap();
            assert!(reg.is_authorized(id, addr(1)).unwrap());

            // Remove from whitelist
            reg.modify_whitelist(admin, id, addr(1), false).unwrap();
            assert!(!reg.is_authorized(id, addr(1)).unwrap());
        });
    }

    #[test]
    fn create_blacklist_policy() {
        with_storage(Box::new(MemStorage::new()), || {
            let mut reg = MIP403Registry::new();
            let admin = addr(99);
            let id = reg.create_policy(admin, PolicyType::Blacklist).unwrap();

            // Not blacklisted -> allowed
            assert!(reg.is_authorized(id, addr(1)).unwrap());

            // Blacklist addr(1)
            reg.modify_blacklist(admin, id, addr(1), true).unwrap();
            assert!(!reg.is_authorized(id, addr(1)).unwrap());

            // Remove from blacklist
            reg.modify_blacklist(admin, id, addr(1), false).unwrap();
            assert!(reg.is_authorized(id, addr(1)).unwrap());
        });
    }

    #[test]
    fn only_admin_can_modify() {
        with_storage(Box::new(MemStorage::new()), || {
            let mut reg = MIP403Registry::new();
            let admin = addr(99);
            let id = reg.create_policy(admin, PolicyType::Whitelist).unwrap();

            let result = reg.modify_whitelist(addr(1), id, addr(2), true);
            assert!(result.is_err());
        });
    }

    #[test]
    fn wrong_type_modify_rejected() {
        with_storage(Box::new(MemStorage::new()), || {
            let mut reg = MIP403Registry::new();
            let admin = addr(99);
            let id = reg.create_policy(admin, PolicyType::Whitelist).unwrap();

            // Try to modify as blacklist -> error
            let result = reg.modify_blacklist(admin, id, addr(1), true);
            assert!(result.is_err());
        });
    }

    #[test]
    fn transfer_authorized_both_parties() {
        with_storage(Box::new(MemStorage::new()), || {
            let mut reg = MIP403Registry::new();
            let admin = addr(99);
            let id = reg.create_policy(admin, PolicyType::Whitelist).unwrap();

            reg.modify_whitelist(admin, id, addr(1), true).unwrap();
            reg.modify_whitelist(admin, id, addr(2), true).unwrap();

            assert!(reg.is_transfer_authorized(id, addr(1), addr(2)).unwrap());

            // Only one party whitelisted -> denied
            assert!(!reg.is_transfer_authorized(id, addr(1), addr(3)).unwrap());
        });
    }

    #[test]
    fn set_policy_admin() {
        with_storage(Box::new(MemStorage::new()), || {
            let mut reg = MIP403Registry::new();
            let admin = addr(99);
            let new_admin = addr(88);
            let id = reg.create_policy(admin, PolicyType::Whitelist).unwrap();

            reg.set_policy_admin(admin, id, new_admin).unwrap();

            // Old admin can no longer modify
            let result = reg.modify_whitelist(admin, id, addr(1), true);
            assert!(result.is_err());

            // New admin can modify
            reg.modify_whitelist(new_admin, id, addr(1), true).unwrap();
            assert!(reg.is_authorized(id, addr(1)).unwrap());
        });
    }
}
