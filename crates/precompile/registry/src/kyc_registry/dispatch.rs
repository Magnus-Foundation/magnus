use crate::{
    Precompile, dispatch_call, input_cost, mutate_void, view,
    kyc_registry::KYCRegistry,
};
use alloy::{primitives::Address, sol_types::SolInterface};
use magnus_contracts::precompiles::IKYCRegistry::IKYCRegistryCalls;
use revm::precompile::{PrecompileError, PrecompileResult};

impl Precompile for KYCRegistry {
    fn call(&mut self, calldata: &[u8], msg_sender: Address) -> PrecompileResult {
        self.storage
            .deduct_gas(input_cost(calldata.len()))
            .map_err(|_| PrecompileError::OutOfGas)?;

        dispatch_call(
            calldata,
            IKYCRegistryCalls::abi_decode,
            |call| match call {
                // View functions
                IKYCRegistryCalls::isVerified(call) => view(call, |c| self.is_verified(c)),
                IKYCRegistryCalls::getKYCLevel(call) => view(call, |c| self.get_kyc_level(c)),
                IKYCRegistryCalls::getKYCRecord(call) => view(call, |c| self.get_kyc_record(c)),
                IKYCRegistryCalls::isVerifier(call) => view(call, |c| self.is_verifier(c)),
                IKYCRegistryCalls::owner(call) => view(call, |_| self.owner()),

                // Verifier functions
                IKYCRegistryCalls::setVerified(call) => {
                    mutate_void(call, msg_sender, |s, c| self.set_verified(s, c))
                }
                IKYCRegistryCalls::revoke(call) => {
                    mutate_void(call, msg_sender, |s, c| self.revoke(s, c))
                }
                IKYCRegistryCalls::batchSetVerified(call) => {
                    mutate_void(call, msg_sender, |s, c| self.batch_set_verified(s, c))
                }

                // Owner functions
                IKYCRegistryCalls::addVerifier(call) => {
                    mutate_void(call, msg_sender, |s, c| self.add_verifier(s, c))
                }
                IKYCRegistryCalls::removeVerifier(call) => {
                    mutate_void(call, msg_sender, |s, c| self.remove_verifier(s, c))
                }
                IKYCRegistryCalls::transferOwnership(call) => {
                    mutate_void(call, msg_sender, |s, c| self.transfer_ownership(s, c))
                }
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        kyc_registry::IKYCRegistry,
        storage::{ContractStorage, StorageCtx, hashmap::HashMapStorageProvider},
    };
    use alloy::{primitives::U256, sol_types::SolCall};

    #[test]
    fn test_set_verified_via_dispatch() -> eyre::Result<()> {
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

            let calldata = IKYCRegistry::setVerifiedCall {
                account,
                level: 2,
                expiry: 5000,
                jurisdiction: 1,
            }
            .abi_encode();

            let result = registry.call(&calldata, verifier);
            assert!(result.is_ok());
            let output = result.unwrap();
            assert!(!output.reverted);

            // Verify via view call
            let query = IKYCRegistry::isVerifiedCall { account }.abi_encode();
            let result = registry.call(&query, Address::ZERO);
            assert!(result.is_ok());
            let output = result.unwrap();
            assert!(!output.reverted);

            let is_verified =
                IKYCRegistry::isVerifiedCall::abi_decode_returns(&output.bytes).unwrap();
            assert!(is_verified);

            Ok(())
        })
    }

    #[test]
    fn test_selector_coverage() -> eyre::Result<()> {
        use crate::test_util::{assert_full_coverage, check_selector_coverage};
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut registry = KYCRegistry::new();

            let unsupported = check_selector_coverage(
                &mut registry,
                IKYCRegistryCalls::SELECTORS,
                "IKYCRegistry",
                IKYCRegistryCalls::name_by_selector,
            );

            assert_full_coverage([unsupported]);
            Ok(())
        })
    }
}
