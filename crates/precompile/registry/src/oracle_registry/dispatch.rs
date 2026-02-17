use crate::{
    Precompile, dispatch_call, input_cost, mutate_void, oracle_registry::OracleRegistry,
    view,
};
use alloy::{primitives::Address, sol_types::SolInterface};
use revm::precompile::{PrecompileError, PrecompileResult};
use magnus_contracts::precompiles::IOracleRegistry::IOracleRegistryCalls;

impl Precompile for OracleRegistry {
    fn call(&mut self, calldata: &[u8], msg_sender: Address) -> PrecompileResult {
        self.storage
            .deduct_gas(input_cost(calldata.len()))
            .map_err(|_| PrecompileError::OutOfGas)?;

        dispatch_call(
            calldata,
            IOracleRegistryCalls::abi_decode,
            |call| match call {
                // View functions
                IOracleRegistryCalls::getRate(call) => view(call, |c| self.get_rate(c)),
                IOracleRegistryCalls::getRateWithTimestamp(call) => view(call, |c| self.get_rate_with_timestamp(c)),
                IOracleRegistryCalls::isReporter(call) => view(call, |c| self.is_reporter(c)),
                IOracleRegistryCalls::isExternalFeed(call) => view(call, |c| self.is_external_feed(c)),
                IOracleRegistryCalls::isFrozen(call) => view(call, |c| self.is_frozen(c)),
                IOracleRegistryCalls::getReportExpiry(call) => view(call, |c| self.get_report_expiry(c)),
                IOracleRegistryCalls::ratePairId(call) => view(call, |c| self.rate_pair_id_view(c)),
                IOracleRegistryCalls::numReports(call) => view(call, |c| self.num_reports(c)),
                IOracleRegistryCalls::owner(call) => view(call, |_| self.owner()),
                // State-changing functions
                IOracleRegistryCalls::report(call) => mutate_void(call, msg_sender, |s, c| self.report(s, c)),
                IOracleRegistryCalls::reportExternal(call) => mutate_void(call, msg_sender, |s, c| self.report_external(s, c)),
                IOracleRegistryCalls::addReporter(call) => mutate_void(call, msg_sender, |s, c| self.add_reporter(s, c)),
                IOracleRegistryCalls::removeReporter(call) => mutate_void(call, msg_sender, |s, c| self.remove_reporter(s, c)),
                IOracleRegistryCalls::addExternalFeed(call) => mutate_void(call, msg_sender, |s, c| self.add_external_feed(s, c)),
                IOracleRegistryCalls::removeExternalFeed(call) => mutate_void(call, msg_sender, |s, c| self.remove_external_feed(s, c)),
                IOracleRegistryCalls::resetBreaker(call) => mutate_void(call, msg_sender, |s, c| self.reset_breaker(s, c)),
                IOracleRegistryCalls::setExpiry(call) => mutate_void(call, msg_sender, |s, c| self.set_expiry(s, c)),
                IOracleRegistryCalls::transferOwnership(call) => mutate_void(call, msg_sender, |s, c| self.transfer_ownership(s, c)),
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
        oracle_registry::IOracleRegistry,
    };
    use alloy::primitives::U256;
    use alloy::sol_types::{SolCall, SolValue};
    use magnus_contracts::precompiles::IOracleRegistry::IOracleRegistryCalls;

    #[test]
    fn test_report_via_dispatch() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let owner = Address::random();
        let reporter = Address::random();
        let base = Address::with_last_byte(10);
        let quote = Address::with_last_byte(20);

        StorageCtx::enter(&mut storage, || {
            let mut registry = OracleRegistry::new();
            registry.initialize(owner)?;

            // Add reporter via dispatch
            let calldata = IOracleRegistry::addReporterCall { reporter }.abi_encode();
            let result = registry.call(&calldata, owner);
            assert!(result.is_ok());

            // Submit report via dispatch
            let calldata = IOracleRegistry::reportCall {
                base, quote, value: U256::from(25500),
            }.abi_encode();
            let result = registry.call(&calldata, reporter);
            assert!(result.is_ok());

            // Get rate via dispatch
            let calldata = IOracleRegistry::getRateCall { base, quote }.abi_encode();
            let result = registry.call(&calldata, Address::ZERO)?;
            let rate = U256::abi_decode(&result.bytes).unwrap();
            assert_eq!(rate, U256::from(25500));

            Ok(())
        })
    }

    #[test]
    fn test_selector_coverage() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut registry = OracleRegistry::new();

            let unsupported = check_selector_coverage(
                &mut registry,
                IOracleRegistryCalls::SELECTORS,
                "IOracleRegistry",
                IOracleRegistryCalls::name_by_selector,
            );

            assert_full_coverage([unsupported]);
            Ok(())
        })
    }
}
