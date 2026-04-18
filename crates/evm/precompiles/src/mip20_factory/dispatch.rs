//! ABI dispatch for the [`MIP20Factory`] precompile.

use crate::{
    Precompile, charge_input_cost, dispatch_call, mutate, mip20_factory::MIP20Factory, view,
};
use alloy::{primitives::Address, sol_types::SolInterface};
use revm::precompile::PrecompileResult;
use magnus_contracts::precompiles::IMIP20Factory::IMIP20FactoryCalls;

impl Precompile for MIP20Factory {
    fn call(&mut self, calldata: &[u8], msg_sender: Address) -> PrecompileResult {
        if let Some(err) = charge_input_cost(&mut self.storage, calldata) {
            return err;
        }

        dispatch_call(
            calldata,
            &[],
            IMIP20FactoryCalls::abi_decode,
            |call| match call {
                IMIP20FactoryCalls::createToken(call) => {
                    mutate(call, msg_sender, |s, c| self.create_token(s, c))
                }
                IMIP20FactoryCalls::isTIP20(call) => view(call, |c| self.is_tip20(c.token)),
                IMIP20FactoryCalls::getTokenAddress(call) => {
                    view(call, |c| self.get_token_address(c))
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
    use magnus_contracts::precompiles::IMIP20Factory::IMIP20FactoryCalls;

    #[test]
    fn mip20_factory_test_selector_coverage() {
        let mut storage = HashMapStorageProvider::new(1);

        StorageCtx::enter(&mut storage, || {
            let mut factory = MIP20Factory::new();

            let unsupported = check_selector_coverage(
                &mut factory,
                IMIP20FactoryCalls::SELECTORS,
                "IMIP20Factory",
                IMIP20FactoryCalls::name_by_selector,
            );

            assert_full_coverage([unsupported]);
        })
    }
}
