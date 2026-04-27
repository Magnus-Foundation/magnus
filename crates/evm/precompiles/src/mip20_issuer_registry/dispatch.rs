//! ABI dispatch for the [`MIP20IssuerRegistry`] precompile.
//!
//! **G0 status:** scaffold only — view selectors route to stub methods returning empty
//! results. Governance-gated state-changing functions (`addApprovedIssuer`,
//! `removeApprovedIssuer`) land in G4.

use crate::{
    Precompile, charge_input_cost, dispatch_call, mip20_issuer_registry::MIP20IssuerRegistry,
    storage::StorageCtx, view,
};
use alloy::{primitives::Address, sol_types::SolInterface};
use revm::precompile::PrecompileResult;
use magnus_contracts::precompiles::IMIP20IssuerRegistry::IMIP20IssuerRegistryCalls;

impl Precompile for MIP20IssuerRegistry {
    fn call(&mut self, calldata: &[u8], _msg_sender: Address) -> PrecompileResult {
        if let Some(err) = charge_input_cost(&mut self.storage, calldata) {
            return err;
        }

        dispatch_call(
            calldata,
            &[],
            IMIP20IssuerRegistryCalls::abi_decode,
            |call| match call {
                IMIP20IssuerRegistryCalls::isApprovedIssuer(call) => {
                    view(call, |c| self.is_approved_issuer(&c.currency, c.issuer))
                }
                IMIP20IssuerRegistryCalls::getApprovedIssuers(call) => {
                    view(call, |c| self.get_approved_issuers(&c.currency))
                }
                // G0: governance-gated mutators are stubbed. Filled in by G4.
                IMIP20IssuerRegistryCalls::addApprovedIssuer(_)
                | IMIP20IssuerRegistryCalls::removeApprovedIssuer(_) => {
                    // Until G4 lands the real EIP-712 governance verification, these
                    // selectors return a deterministic "fatal: not implemented" error so
                    // callers can detect the unimplemented state without confusing it for
                    // a normal business-logic revert.
                    StorageCtx.error_result(crate::error::MagnusPrecompileError::Fatal(
                        "MIP20IssuerRegistry governance functions not implemented in G0".into(),
                    ))
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
    };
    use magnus_contracts::precompiles::IMIP20IssuerRegistry::IMIP20IssuerRegistryCalls;

    #[test]
    fn mip20_issuer_registry_test_selector_coverage() {
        let mut storage = HashMapStorageProvider::new(1);

        StorageCtx::enter(&mut storage, || {
            let mut registry = MIP20IssuerRegistry::new();

            let unsupported = check_selector_coverage(
                &mut registry,
                IMIP20IssuerRegistryCalls::SELECTORS,
                "IMIP20IssuerRegistry",
                IMIP20IssuerRegistryCalls::name_by_selector,
            );

            assert_full_coverage([unsupported]);
        })
    }
}
