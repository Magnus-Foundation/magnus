use crate::{Precompile, dispatch_call, input_cost, mutate, mip20_factory::MIP20Factory, view};
use alloy::{primitives::Address, sol_types::SolInterface};
use revm::precompile::{PrecompileError, PrecompileResult};
use magnus_contracts::precompiles::IMIP20Factory::IMIP20FactoryCalls;

impl Precompile for MIP20Factory {
    fn call(&mut self, calldata: &[u8], msg_sender: Address) -> PrecompileResult {
        self.storage
            .deduct_gas(input_cost(calldata.len()))
            .map_err(|_| PrecompileError::OutOfGas)?;

        dispatch_call(
            calldata,
            IMIP20FactoryCalls::abi_decode,
            |call| match call {
                IMIP20FactoryCalls::createToken(call) => {
                    mutate(call, msg_sender, |s, c| self.create_token(s, c))
                }
                IMIP20FactoryCalls::isMIP20(call) => view(call, |c| self.is_mip20(c.token)),
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
