use crate::{
    Precompile, dispatch_call, input_cost, mutate, mutate_void, mip403_registry::MIP403Registry,
    view,
};
use alloy::{primitives::Address, sol_types::SolInterface};
use revm::precompile::{PrecompileError, PrecompileResult};
use magnus_contracts::precompiles::IMIP403Registry::IMIP403RegistryCalls;

impl Precompile for MIP403Registry {
    fn call(&mut self, calldata: &[u8], msg_sender: Address) -> PrecompileResult {
        self.storage
            .deduct_gas(input_cost(calldata.len()))
            .map_err(|_| PrecompileError::OutOfGas)?;

        dispatch_call(
            calldata,
            IMIP403RegistryCalls::abi_decode,
            |call| match call {
                IMIP403RegistryCalls::policyIdCounter(call) => {
                    view(call, |_| self.policy_id_counter())
                }
                IMIP403RegistryCalls::policyExists(call) => view(call, |c| self.policy_exists(c)),
                IMIP403RegistryCalls::policyData(call) => view(call, |c| self.policy_data(c)),
                IMIP403RegistryCalls::isAuthorized(call) => view(call, |c| self.is_authorized(c)),
                IMIP403RegistryCalls::createPolicy(call) => {
                    mutate(call, msg_sender, |s, c| self.create_policy(s, c))
                }
                IMIP403RegistryCalls::createPolicyWithAccounts(call) => {
                    mutate(call, msg_sender, |s, c| {
                        self.create_policy_with_accounts(s, c)
                    })
                }
                IMIP403RegistryCalls::setPolicyAdmin(call) => {
                    mutate_void(call, msg_sender, |s, c| self.set_policy_admin(s, c))
                }
                IMIP403RegistryCalls::modifyPolicyWhitelist(call) => {
                    mutate_void(call, msg_sender, |s, c| self.modify_policy_whitelist(s, c))
                }
                IMIP403RegistryCalls::modifyPolicyBlacklist(call) => {
                    mutate_void(call, msg_sender, |s, c| self.modify_policy_blacklist(s, c))
                }
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        storage::{StorageCtx, hashmap::HashMapStorageProvider},
        test_util::{assert_full_coverage, check_selector_coverage},
        mip403_registry::IMIP403Registry,
    };
    use alloy::sol_types::{SolCall, SolValue};
    use magnus_contracts::precompiles::IMIP403Registry::IMIP403RegistryCalls;

    #[test]
    fn test_is_authorized_precompile() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let user = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut registry = MIP403Registry::new();

            // Test policy 1 (always allow)
            let call = IMIP403Registry::isAuthorizedCall { policyId: 1, user };
            let calldata = call.abi_encode();
            let result = registry.call(&calldata, Address::ZERO);

            assert!(result.is_ok());
            let output = result.unwrap();
            let decoded: bool =
                IMIP403Registry::isAuthorizedCall::abi_decode_returns(&output.bytes).unwrap();
            assert!(decoded);

            Ok(())
        })
    }

    #[test]
    fn test_create_policy_precompile() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut registry = MIP403Registry::new();

            let call = IMIP403Registry::createPolicyCall {
                admin,
                policyType: IMIP403Registry::PolicyType::WHITELIST,
            };
            let calldata = call.abi_encode();
            let result = registry.call(&calldata, admin);

            assert!(result.is_ok());
            let output = result.unwrap();
            let decoded: u64 =
                IMIP403Registry::createPolicyCall::abi_decode_returns(&output.bytes).unwrap();
            assert_eq!(decoded, 2); // First created policy ID

            Ok(())
        })
    }

    #[test]
    fn test_policy_id_counter_initialization() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let sender = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut registry = MIP403Registry::new();

            // Get initial counter
            let counter_call = IMIP403Registry::policyIdCounterCall {};
            let calldata = counter_call.abi_encode();
            let result = registry.call(&calldata, sender).unwrap();
            let counter = u64::abi_decode(&result.bytes).unwrap();
            assert_eq!(counter, 2); // Counter starts at 2 (policies 0 and 1 are reserved)

            Ok(())
        })
    }

    #[test]
    fn test_create_policy_with_accounts() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let account1 = Address::random();
        let account2 = Address::random();
        let other_account = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut registry = MIP403Registry::new();

            let accounts = vec![account1, account2];
            let call = IMIP403Registry::createPolicyWithAccountsCall {
                admin,
                policyType: IMIP403Registry::PolicyType::WHITELIST,
                accounts,
            };
            let calldata = call.abi_encode();
            let result = registry.call(&calldata, admin).unwrap();

            let policy_id: u64 =
                IMIP403Registry::createPolicyWithAccountsCall::abi_decode_returns(&result.bytes)
                    .unwrap();
            assert_eq!(policy_id, 2);

            // Check that accounts are authorized
            let is_auth_call = IMIP403Registry::isAuthorizedCall {
                policyId: policy_id,
                user: account1,
            };
            let calldata = is_auth_call.abi_encode();
            let result = registry.call(&calldata, admin).unwrap();
            let is_authorized = bool::abi_decode(&result.bytes).unwrap();
            assert!(is_authorized);

            let is_auth_call = IMIP403Registry::isAuthorizedCall {
                policyId: policy_id,
                user: account2,
            };
            let calldata = is_auth_call.abi_encode();
            let result = registry.call(&calldata, admin).unwrap();
            let is_authorized = bool::abi_decode(&result.bytes).unwrap();
            assert!(is_authorized);

            // Check that other accounts are not authorized
            let is_auth_call = IMIP403Registry::isAuthorizedCall {
                policyId: policy_id,
                user: other_account,
            };
            let calldata = is_auth_call.abi_encode();
            let result = registry.call(&calldata, admin).unwrap();
            let is_authorized = bool::abi_decode(&result.bytes).unwrap();
            assert!(!is_authorized);

            Ok(())
        })
    }

    #[test]
    fn test_blacklist_policy() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let blocked_account = Address::random();
        let allowed_account = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut registry = MIP403Registry::new();

            // Create blacklist policy
            let call = IMIP403Registry::createPolicyCall {
                admin,
                policyType: IMIP403Registry::PolicyType::BLACKLIST,
            };
            let calldata = call.abi_encode();
            let result = registry.call(&calldata, admin).unwrap();
            let policy_id: u64 =
                IMIP403Registry::createPolicyCall::abi_decode_returns(&result.bytes).unwrap();

            // Initially, all accounts should be authorized (empty blacklist)
            let is_auth_call = IMIP403Registry::isAuthorizedCall {
                policyId: policy_id,
                user: blocked_account,
            };
            let calldata = is_auth_call.abi_encode();
            let result = registry.call(&calldata, admin).unwrap();
            let is_authorized = bool::abi_decode(&result.bytes).unwrap();
            assert!(is_authorized);

            // Add account to blacklist
            let modify_call = IMIP403Registry::modifyPolicyBlacklistCall {
                policyId: policy_id,
                account: blocked_account,
                restricted: true,
            };
            let calldata = modify_call.abi_encode();
            registry.call(&calldata, admin).unwrap();

            // Now blocked account should not be authorized
            let is_auth_call = IMIP403Registry::isAuthorizedCall {
                policyId: policy_id,
                user: blocked_account,
            };
            let calldata = is_auth_call.abi_encode();
            let result = registry.call(&calldata, admin).unwrap();
            let is_authorized = bool::abi_decode(&result.bytes).unwrap();
            assert!(!is_authorized);

            // Other accounts should still be authorized
            let is_auth_call = IMIP403Registry::isAuthorizedCall {
                policyId: policy_id,
                user: allowed_account,
            };
            let calldata = is_auth_call.abi_encode();
            let result = registry.call(&calldata, admin).unwrap();
            let is_authorized = bool::abi_decode(&result.bytes).unwrap();
            assert!(is_authorized);

            // Remove account from blacklist
            let modify_call = IMIP403Registry::modifyPolicyBlacklistCall {
                policyId: policy_id,
                account: blocked_account,
                restricted: false,
            };
            let calldata = modify_call.abi_encode();
            registry.call(&calldata, admin).unwrap();

            // Account should be authorized again
            let is_auth_call = IMIP403Registry::isAuthorizedCall {
                policyId: policy_id,
                user: blocked_account,
            };
            let calldata = is_auth_call.abi_encode();
            let result = registry.call(&calldata, admin).unwrap();
            let is_authorized = bool::abi_decode(&result.bytes).unwrap();
            assert!(is_authorized);

            Ok(())
        })
    }

    #[test]
    fn test_modify_policy_whitelist() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let account1 = Address::random();
        let account2 = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut registry = MIP403Registry::new();

            // Create whitelist policy
            let call = IMIP403Registry::createPolicyCall {
                admin,
                policyType: IMIP403Registry::PolicyType::WHITELIST,
            };
            let calldata = call.abi_encode();
            let result = registry.call(&calldata, admin).unwrap();
            let policy_id: u64 =
                IMIP403Registry::createPolicyCall::abi_decode_returns(&result.bytes).unwrap();

            // Add multiple accounts to whitelist
            let modify_call1 = IMIP403Registry::modifyPolicyWhitelistCall {
                policyId: policy_id,
                account: account1,
                allowed: true,
            };
            let calldata = modify_call1.abi_encode();
            registry.call(&calldata, admin).unwrap();

            let modify_call2 = IMIP403Registry::modifyPolicyWhitelistCall {
                policyId: policy_id,
                account: account2,
                allowed: true,
            };
            let calldata = modify_call2.abi_encode();
            registry.call(&calldata, admin).unwrap();

            // Both accounts should be authorized
            let is_auth_call = IMIP403Registry::isAuthorizedCall {
                policyId: policy_id,
                user: account1,
            };
            let calldata = is_auth_call.abi_encode();
            let result = registry.call(&calldata, admin).unwrap();
            let is_authorized = bool::abi_decode(&result.bytes).unwrap();
            assert!(is_authorized);

            let is_auth_call = IMIP403Registry::isAuthorizedCall {
                policyId: policy_id,
                user: account2,
            };
            let calldata = is_auth_call.abi_encode();
            let result = registry.call(&calldata, admin).unwrap();
            let is_authorized = bool::abi_decode(&result.bytes).unwrap();
            assert!(is_authorized);

            // Remove one account from whitelist
            let modify_call = IMIP403Registry::modifyPolicyWhitelistCall {
                policyId: policy_id,
                account: account1,
                allowed: false,
            };
            let calldata = modify_call.abi_encode();
            registry.call(&calldata, admin).unwrap();

            // Account1 should not be authorized, account2 should still be
            let is_auth_call = IMIP403Registry::isAuthorizedCall {
                policyId: policy_id,
                user: account1,
            };
            let calldata = is_auth_call.abi_encode();
            let result = registry.call(&calldata, admin).unwrap();
            let is_authorized = bool::abi_decode(&result.bytes).unwrap();
            assert!(!is_authorized);

            let is_auth_call = IMIP403Registry::isAuthorizedCall {
                policyId: policy_id,
                user: account2,
            };
            let calldata = is_auth_call.abi_encode();
            let result = registry.call(&calldata, admin).unwrap();
            let is_authorized = bool::abi_decode(&result.bytes).unwrap();
            assert!(is_authorized);

            Ok(())
        })
    }

    #[test]
    fn test_set_policy_admin() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let new_admin = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut registry = MIP403Registry::new();

            // Create a policy
            let call = IMIP403Registry::createPolicyCall {
                admin,
                policyType: IMIP403Registry::PolicyType::WHITELIST,
            };
            let calldata = call.abi_encode();
            let result = registry.call(&calldata, admin).unwrap();
            let policy_id: u64 =
                IMIP403Registry::createPolicyCall::abi_decode_returns(&result.bytes).unwrap();

            // Get initial policy data
            let policy_data_call = IMIP403Registry::policyDataCall {
                policyId: policy_id,
            };
            let calldata = policy_data_call.abi_encode();
            let result = registry.call(&calldata, admin).unwrap();
            let policy_data =
                IMIP403Registry::policyDataCall::abi_decode_returns(&result.bytes).unwrap();
            assert_eq!(policy_data.admin, admin);

            // Change policy admin
            let set_admin_call = IMIP403Registry::setPolicyAdminCall {
                policyId: policy_id,
                admin: new_admin,
            };
            let calldata = set_admin_call.abi_encode();
            registry.call(&calldata, admin).unwrap();

            // Verify policy admin was changed
            let policy_data_call = IMIP403Registry::policyDataCall {
                policyId: policy_id,
            };
            let calldata = policy_data_call.abi_encode();
            let result = registry.call(&calldata, admin).unwrap();
            let policy_data =
                IMIP403Registry::policyDataCall::abi_decode_returns(&result.bytes).unwrap();
            assert_eq!(policy_data.admin, new_admin);

            Ok(())
        })
    }

    #[test]
    fn test_special_policy_ids() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let user = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut registry = MIP403Registry::new();

            // Test policy 0 (always deny)
            let is_auth_call = IMIP403Registry::isAuthorizedCall { policyId: 0, user };
            let calldata = is_auth_call.abi_encode();
            let result = registry.call(&calldata, Address::ZERO).unwrap();
            let is_authorized = bool::abi_decode(&result.bytes).unwrap();
            assert!(!is_authorized);

            // Test policy 1 (always allow)
            let is_auth_call = IMIP403Registry::isAuthorizedCall { policyId: 1, user };
            let calldata = is_auth_call.abi_encode();
            let result = registry.call(&calldata, Address::ZERO).unwrap();
            let is_authorized = bool::abi_decode(&result.bytes).unwrap();
            assert!(is_authorized);

            Ok(())
        })
    }

    #[test]
    fn test_invalid_selector() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let sender = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut registry = MIP403Registry::new();

            // Test with invalid selector - should return Ok with reverted status
            let invalid_data = vec![0x12, 0x34, 0x56, 0x78];
            let result = registry.call(&invalid_data, sender);
            assert!(result.is_ok());
            assert!(result.unwrap().reverted);

            // Test with insufficient data
            let short_data = vec![0x12, 0x34];
            let result = registry.call(&short_data, sender);
            assert!(result.is_err());

            Ok(())
        })
    }

    #[test]
    fn test_create_multiple_policies() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut registry = MIP403Registry::new();

            // Create multiple policies with different types
            let whitelist_call = IMIP403Registry::createPolicyCall {
                admin,
                policyType: IMIP403Registry::PolicyType::WHITELIST,
            };
            let calldata = whitelist_call.abi_encode();
            let result = registry.call(&calldata, admin).unwrap();
            let whitelist_id: u64 =
                IMIP403Registry::createPolicyCall::abi_decode_returns(&result.bytes).unwrap();

            let blacklist_call = IMIP403Registry::createPolicyCall {
                admin,
                policyType: IMIP403Registry::PolicyType::BLACKLIST,
            };
            let calldata = blacklist_call.abi_encode();
            let result = registry.call(&calldata, admin).unwrap();
            let blacklist_id: u64 =
                IMIP403Registry::createPolicyCall::abi_decode_returns(&result.bytes).unwrap();

            // Verify IDs are sequential
            assert_eq!(whitelist_id, 2);
            assert_eq!(blacklist_id, 3);

            // Verify counter has been updated
            let counter_call = IMIP403Registry::policyIdCounterCall {};
            let calldata = counter_call.abi_encode();
            let result = registry.call(&calldata, admin).unwrap();
            let counter = u64::abi_decode(&result.bytes).unwrap();
            assert_eq!(counter, 4);

            Ok(())
        })
    }

    #[test]
    fn test_selector_coverage() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut registry = MIP403Registry::new();

            let unsupported = check_selector_coverage(
                &mut registry,
                IMIP403RegistryCalls::SELECTORS,
                "IMIP403Registry",
                IMIP403RegistryCalls::name_by_selector,
            );

            assert_full_coverage([unsupported]);

            Ok(())
        })
    }
}
