pub mod dispatch;

pub use magnus_contracts::precompiles::{IOracleRegistry, OracleRegistryError, OracleRegistryEvent};
use magnus_precompile_macros::{Storable, contract};

use crate::{
    ORACLE_REGISTRY_ADDRESS,
    error::{Result, MagnusPrecompileError},
    storage::{Handler, Mapping},
};
use alloy::primitives::{Address, B256, U256, keccak256};

/// Default report expiry in seconds.
pub const DEFAULT_REPORT_EXPIRY: u64 = 360;
/// Circuit breaker threshold: 20% deviation in basis points.
pub const BREAKER_THRESHOLD_BPS: u64 = 2000;
pub const BPS_DENOMINATOR: u64 = 10000;
/// Maximum reporters per pair (indexed storage constraint).
pub const MAX_REPORTERS_PER_PAIR: u8 = 32;

/// Compute the rate pair ID from base and quote token addresses.
pub fn rate_pair_id(base: Address, quote: Address) -> B256 {
    let mut data = Vec::with_capacity(40);
    data.extend_from_slice(base.as_slice());
    data.extend_from_slice(quote.as_slice());
    keccak256(data)
}

#[derive(Debug, Clone, Storable)]
pub struct OracleReport {
    pub reporter: Address,
    pub value: U256,
    pub timestamp: u64,
}

#[contract(addr = ORACLE_REGISTRY_ADDRESS)]
pub struct OracleRegistry {
    owner: Address,
    reporters: Mapping<Address, bool>,
    external_feeds: Mapping<Address, bool>,
    // Reports stored as: pair_id -> reporter_index -> report
    reports: Mapping<B256, Mapping<u8, OracleReport>>,
    report_count: Mapping<B256, u8>,
    // Reporter address -> index in the pair's report array
    reporter_index: Mapping<B256, Mapping<Address, u8>>,
    reporter_has_report: Mapping<B256, Mapping<Address, bool>>,
    frozen_pairs: Mapping<B256, bool>,
    expiry_overrides: Mapping<B256, u64>,
    // Track last update timestamp per pair for getRateWithTimestamp
    last_updated: Mapping<B256, u64>,
}

impl OracleRegistry {
    pub fn initialize(&mut self, init_owner: Address) -> Result<()> {
        self.__initialize()?;
        self.owner.write(init_owner)?;
        Ok(())
    }

    // --- View functions ---

    pub fn owner(&self) -> Result<Address> {
        self.owner.read()
    }

    pub fn is_reporter(&self, call: IOracleRegistry::isReporterCall) -> Result<bool> {
        self.reporters[call.reporter].read()
    }

    pub fn is_external_feed(&self, call: IOracleRegistry::isExternalFeedCall) -> Result<bool> {
        self.external_feeds[call.feed].read()
    }

    pub fn is_frozen(&self, call: IOracleRegistry::isFrozenCall) -> Result<bool> {
        self.frozen_pairs[call.pairId].read()
    }

    pub fn get_report_expiry(&self, call: IOracleRegistry::getReportExpiryCall) -> Result<u64> {
        let override_val = self.expiry_overrides[call.pairId].read()?;
        if override_val > 0 {
            Ok(override_val)
        } else {
            Ok(DEFAULT_REPORT_EXPIRY)
        }
    }

    pub fn rate_pair_id_view(&self, call: IOracleRegistry::ratePairIdCall) -> Result<B256> {
        Ok(rate_pair_id(call.base, call.quote))
    }

    pub fn num_reports(&self, call: IOracleRegistry::numReportsCall) -> Result<u8> {
        let pair_id = rate_pair_id(call.base, call.quote);
        self.report_count[pair_id].read()
    }

    pub fn get_rate(&self, call: IOracleRegistry::getRateCall) -> Result<U256> {
        let pair_id = rate_pair_id(call.base, call.quote);
        self.compute_median(pair_id)
    }

    pub fn get_rate_with_timestamp(
        &self,
        call: IOracleRegistry::getRateWithTimestampCall,
    ) -> Result<IOracleRegistry::getRateWithTimestampReturn> {
        let pair_id = rate_pair_id(call.base, call.quote);
        let rate = self.compute_median(pair_id)?;
        let timestamp = self.last_updated[pair_id].read()?;
        Ok(IOracleRegistry::getRateWithTimestampReturn { rate, timestamp })
    }

    // --- State-changing functions ---

    pub fn report(
        &mut self,
        msg_sender: Address,
        call: IOracleRegistry::reportCall,
    ) -> Result<()> {
        // Only whitelisted reporters
        if !self.reporters[msg_sender].read()? {
            return Err(OracleRegistryError::unauthorized().into());
        }
        self.submit_report(msg_sender, call.base, call.quote, call.value)
    }

    pub fn report_external(
        &mut self,
        msg_sender: Address,
        call: IOracleRegistry::reportExternalCall,
    ) -> Result<()> {
        // Only whitelisted external feeds
        if !self.external_feeds[msg_sender].read()? {
            return Err(OracleRegistryError::unauthorized().into());
        }
        self.submit_report(msg_sender, call.base, call.quote, call.value)
    }

    pub fn add_reporter(
        &mut self,
        msg_sender: Address,
        call: IOracleRegistry::addReporterCall,
    ) -> Result<()> {
        self.require_owner(msg_sender)?;
        self.reporters[call.reporter].write(true)?;
        self.emit_event(OracleRegistryEvent::ReporterAdded(
            IOracleRegistry::ReporterAdded {
                reporter: call.reporter,
                addedBy: msg_sender,
            },
        ))
    }

    pub fn remove_reporter(
        &mut self,
        msg_sender: Address,
        call: IOracleRegistry::removeReporterCall,
    ) -> Result<()> {
        self.require_owner(msg_sender)?;
        self.reporters[call.reporter].write(false)?;
        self.emit_event(OracleRegistryEvent::ReporterRemoved(
            IOracleRegistry::ReporterRemoved {
                reporter: call.reporter,
                removedBy: msg_sender,
            },
        ))
    }

    pub fn add_external_feed(
        &mut self,
        msg_sender: Address,
        call: IOracleRegistry::addExternalFeedCall,
    ) -> Result<()> {
        self.require_owner(msg_sender)?;
        self.external_feeds[call.feed].write(true)?;
        self.emit_event(OracleRegistryEvent::ExternalFeedAdded(
            IOracleRegistry::ExternalFeedAdded {
                feed: call.feed,
                addedBy: msg_sender,
            },
        ))
    }

    pub fn remove_external_feed(
        &mut self,
        msg_sender: Address,
        call: IOracleRegistry::removeExternalFeedCall,
    ) -> Result<()> {
        self.require_owner(msg_sender)?;
        self.external_feeds[call.feed].write(false)?;
        self.emit_event(OracleRegistryEvent::ExternalFeedRemoved(
            IOracleRegistry::ExternalFeedRemoved {
                feed: call.feed,
                removedBy: msg_sender,
            },
        ))
    }

    pub fn reset_breaker(
        &mut self,
        msg_sender: Address,
        call: IOracleRegistry::resetBreakerCall,
    ) -> Result<()> {
        self.require_owner(msg_sender)?;
        self.frozen_pairs[call.pairId].write(false)?;
        self.emit_event(OracleRegistryEvent::BreakerReset(
            IOracleRegistry::BreakerReset {
                pairId: call.pairId,
                resetter: msg_sender,
            },
        ))
    }

    pub fn set_expiry(
        &mut self,
        msg_sender: Address,
        call: IOracleRegistry::setExpiryCall,
    ) -> Result<()> {
        self.require_owner(msg_sender)?;
        if call.expiry < 60 {
            return Err(OracleRegistryError::expiry_too_short().into());
        }
        self.expiry_overrides[call.pairId].write(call.expiry)?;
        Ok(())
    }

    pub fn transfer_ownership(
        &mut self,
        msg_sender: Address,
        call: IOracleRegistry::transferOwnershipCall,
    ) -> Result<()> {
        self.require_owner(msg_sender)?;
        let previous = self.owner.read()?;
        self.owner.write(call.newOwner)?;
        self.emit_event(OracleRegistryEvent::OwnershipTransferred(
            IOracleRegistry::OwnershipTransferred {
                previousOwner: previous,
                newOwner: call.newOwner,
            },
        ))
    }

    // --- Internal helpers ---

    fn require_owner(&self, sender: Address) -> Result<()> {
        if sender != self.owner.read()? {
            return Err(OracleRegistryError::unauthorized().into());
        }
        Ok(())
    }

    fn submit_report(
        &mut self,
        reporter: Address,
        base: Address,
        quote: Address,
        value: U256,
    ) -> Result<()> {
        let pair_id = rate_pair_id(base, quote);

        // Check circuit breaker
        if self.frozen_pairs[pair_id].read()? {
            return Err(OracleRegistryError::pair_frozen().into());
        }

        // Check deviation from current median (circuit breaker logic)
        let count = self.report_count[pair_id].read()?;
        if count > 0 {
            if let Ok(current_median) = self.compute_median(pair_id) {
                if !current_median.is_zero() {
                    let deviation = if value > current_median {
                        (value - current_median) * U256::from(BPS_DENOMINATOR) / current_median
                    } else {
                        (current_median - value) * U256::from(BPS_DENOMINATOR) / current_median
                    };
                    if deviation > U256::from(BREAKER_THRESHOLD_BPS) {
                        self.frozen_pairs[pair_id].write(true)?;
                        self.emit_event(OracleRegistryEvent::BreakerTripped(
                            IOracleRegistry::BreakerTripped {
                                pairId: pair_id,
                                reportedValue: value,
                                medianValue: current_median,
                            },
                        ))?;
                        return Err(OracleRegistryError::breaker_exceeded().into());
                    }
                }
            }
        }

        // Get or assign reporter index for this pair
        let idx = if self.reporter_has_report[pair_id][reporter].read()? {
            // Update existing report
            self.reporter_index[pair_id][reporter].read()?
        } else {
            // New reporter for this pair
            let new_idx = count;
            if new_idx >= MAX_REPORTERS_PER_PAIR {
                return Err(OracleRegistryError::unauthorized().into());
            }
            self.reporter_index[pair_id][reporter].write(new_idx)?;
            self.reporter_has_report[pair_id][reporter].write(true)?;
            self.report_count[pair_id].write(new_idx + 1)?;
            new_idx
        };

        // Get block timestamp from storage context
        let timestamp: u64 = self.storage.timestamp().try_into().unwrap_or(u64::MAX);

        // Write the report
        self.reports[pair_id][idx].write(OracleReport {
            reporter,
            value,
            timestamp,
        })?;

        self.last_updated[pair_id].write(timestamp)?;

        self.emit_event(OracleRegistryEvent::RateReported(
            IOracleRegistry::RateReported {
                pairId: pair_id,
                reporter,
                value,
                timestamp,
            },
        ))
    }

    fn compute_median(&self, pair_id: B256) -> Result<U256> {
        let count = self.report_count[pair_id].read()?;
        if count == 0 {
            return Err(OracleRegistryError::no_reports().into());
        }

        let expiry_override = self.expiry_overrides[pair_id].read()?;
        let expiry = if expiry_override > 0 { expiry_override } else { DEFAULT_REPORT_EXPIRY };
        let current_time: u64 = self.storage.timestamp().try_into().unwrap_or(u64::MAX);

        // Collect non-expired values
        let mut values = Vec::new();
        for i in 0..count {
            let report = self.reports[pair_id][i].read()?;
            if current_time.saturating_sub(report.timestamp) < expiry {
                values.push(report.value);
            }
        }

        if values.is_empty() {
            return Err(OracleRegistryError::all_expired().into());
        }

        // Sort and return median
        values.sort();
        let mid = values.len() / 2;
        Ok(values[mid])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{StorageCtx, hashmap::HashMapStorageProvider};

    fn setup() -> HashMapStorageProvider {
        HashMapStorageProvider::new(1)
    }

    #[test]
    fn test_add_reporter_and_report() -> eyre::Result<()> {
        let mut storage = setup();
        let owner = Address::random();
        let reporter = Address::random();
        let base = Address::with_last_byte(10);
        let quote = Address::with_last_byte(20);

        StorageCtx::enter(&mut storage, || {
            let mut registry = OracleRegistry::new();
            registry.initialize(owner)?;

            // Add reporter
            registry.add_reporter(owner, IOracleRegistry::addReporterCall { reporter })?;
            assert!(registry.is_reporter(IOracleRegistry::isReporterCall { reporter })?);

            // Submit report
            registry.report(
                reporter,
                IOracleRegistry::reportCall { base, quote, value: U256::from(25500) },
            )?;

            // Get rate
            let rate = registry.get_rate(IOracleRegistry::getRateCall { base, quote })?;
            assert_eq!(rate, U256::from(25500));

            Ok(())
        })
    }

    #[test]
    fn test_non_reporter_rejected() -> eyre::Result<()> {
        let mut storage = setup();
        let owner = Address::random();
        let non_reporter = Address::random();

        StorageCtx::enter(&mut storage, || {
            let mut registry = OracleRegistry::new();
            registry.initialize(owner)?;

            let result = registry.report(
                non_reporter,
                IOracleRegistry::reportCall {
                    base: Address::with_last_byte(10),
                    quote: Address::with_last_byte(20),
                    value: U256::from(100),
                },
            );
            assert!(result.is_err());
            Ok(())
        })
    }

    #[test]
    fn test_circuit_breaker() -> eyre::Result<()> {
        let mut storage = setup();
        let owner = Address::random();
        let r1 = Address::with_last_byte(1);
        let r2 = Address::with_last_byte(2);
        let base = Address::with_last_byte(10);
        let quote = Address::with_last_byte(20);

        StorageCtx::enter(&mut storage, || {
            let mut registry = OracleRegistry::new();
            registry.initialize(owner)?;
            registry.add_reporter(owner, IOracleRegistry::addReporterCall { reporter: r1 })?;
            registry.add_reporter(owner, IOracleRegistry::addReporterCall { reporter: r2 })?;

            // First report: 1000
            registry.report(r1, IOracleRegistry::reportCall { base, quote, value: U256::from(1000) })?;

            // Second report: 1500 (50% deviation > 20% threshold)
            let result = registry.report(r2, IOracleRegistry::reportCall { base, quote, value: U256::from(1500) });
            assert!(result.is_err());

            // Pair should be frozen
            let pair_id = rate_pair_id(base, quote);
            assert!(registry.is_frozen(IOracleRegistry::isFrozenCall { pairId: pair_id })?);

            // Reset breaker
            registry.reset_breaker(owner, IOracleRegistry::resetBreakerCall { pairId: pair_id })?;
            assert!(!registry.is_frozen(IOracleRegistry::isFrozenCall { pairId: pair_id })?);

            Ok(())
        })
    }

    #[test]
    fn test_external_feed() -> eyre::Result<()> {
        let mut storage = setup();
        let owner = Address::random();
        let feed = Address::random();
        let base = Address::with_last_byte(10);
        let quote = Address::with_last_byte(20);

        StorageCtx::enter(&mut storage, || {
            let mut registry = OracleRegistry::new();
            registry.initialize(owner)?;
            registry.add_external_feed(owner, IOracleRegistry::addExternalFeedCall { feed })?;

            registry.report_external(
                feed,
                IOracleRegistry::reportExternalCall { base, quote, value: U256::from(100) },
            )?;

            let rate = registry.get_rate(IOracleRegistry::getRateCall { base, quote })?;
            assert_eq!(rate, U256::from(100));
            Ok(())
        })
    }

    #[test]
    fn test_median_multiple_reporters() -> eyre::Result<()> {
        let mut storage = setup();
        let owner = Address::random();
        let base = Address::with_last_byte(10);
        let quote = Address::with_last_byte(20);

        StorageCtx::enter(&mut storage, || {
            let mut registry = OracleRegistry::new();
            registry.initialize(owner)?;

            // Add 5 reporters
            let reporters: Vec<Address> = (1..=5).map(|i| Address::with_last_byte(i)).collect();
            for &r in &reporters {
                registry.add_reporter(owner, IOracleRegistry::addReporterCall { reporter: r })?;
            }

            // Reports: 100, 102, 105, 103, 101
            let values = [100u64, 102, 105, 103, 101];
            for (r, v) in reporters.iter().zip(values.iter()) {
                registry.report(*r, IOracleRegistry::reportCall { base, quote, value: U256::from(*v) })?;
            }

            // Sorted: [100, 101, 102, 103, 105], median = 102
            let rate = registry.get_rate(IOracleRegistry::getRateCall { base, quote })?;
            assert_eq!(rate, U256::from(102));
            Ok(())
        })
    }

    #[test]
    fn test_ownership_transfer() -> eyre::Result<()> {
        let mut storage = setup();
        let owner = Address::random();
        let new_owner = Address::random();
        let reporter = Address::random();

        StorageCtx::enter(&mut storage, || {
            let mut registry = OracleRegistry::new();
            registry.initialize(owner)?;

            // Transfer ownership
            registry.transfer_ownership(owner, IOracleRegistry::transferOwnershipCall { newOwner: new_owner })?;

            // Old owner can no longer add reporters
            let result = registry.add_reporter(owner, IOracleRegistry::addReporterCall { reporter });
            assert!(result.is_err());

            // New owner can
            registry.add_reporter(new_owner, IOracleRegistry::addReporterCall { reporter })?;
            Ok(())
        })
    }
}
