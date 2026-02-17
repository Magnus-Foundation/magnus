pub use IOracleRegistry::{
    IOracleRegistryErrors as OracleRegistryError,
    IOracleRegistryEvents as OracleRegistryEvent,
};

crate::sol! {
    #[derive(Debug, PartialEq, Eq)]
    #[sol(abi)]
    interface IOracleRegistry {
        // View Functions
        function getRate(address base, address quote) external view returns (uint256);
        function getRateWithTimestamp(address base, address quote) external view returns (uint256 rate, uint64 timestamp);
        function isReporter(address reporter) external view returns (bool);
        function isExternalFeed(address feed) external view returns (bool);
        function isFrozen(bytes32 pairId) external view returns (bool);
        function getReportExpiry(bytes32 pairId) external view returns (uint64);
        function ratePairId(address base, address quote) external pure returns (bytes32);
        function numReports(address base, address quote) external view returns (uint8);
        function owner() external view returns (address);

        // Reporter functions
        function report(address base, address quote, uint256 value) external;
        function reportExternal(address base, address quote, uint256 value) external;

        // Owner functions
        function addReporter(address reporter) external;
        function removeReporter(address reporter) external;
        function addExternalFeed(address feed) external;
        function removeExternalFeed(address feed) external;
        function resetBreaker(bytes32 pairId) external;
        function setExpiry(bytes32 pairId, uint64 expiry) external;
        function transferOwnership(address newOwner) external;

        // Events
        event RateReported(bytes32 indexed pairId, address indexed reporter, uint256 value, uint64 timestamp);
        event BreakerTripped(bytes32 indexed pairId, uint256 reportedValue, uint256 medianValue);
        event BreakerReset(bytes32 indexed pairId, address indexed resetter);
        event ReporterAdded(address indexed reporter, address indexed addedBy);
        event ReporterRemoved(address indexed reporter, address indexed removedBy);
        event ExternalFeedAdded(address indexed feed, address indexed addedBy);
        event ExternalFeedRemoved(address indexed feed, address indexed removedBy);
        event OwnershipTransferred(address indexed previousOwner, address indexed newOwner);

        // Errors
        error Unauthorized();
        error PairFrozen();
        error NoReportsAvailable();
        error AllReportsExpired();
        error ExpiryTooShort();
        error BreakerThresholdExceeded();
    }
}

impl OracleRegistryError {
    pub const fn unauthorized() -> Self {
        Self::Unauthorized(IOracleRegistry::Unauthorized {})
    }
    pub const fn pair_frozen() -> Self {
        Self::PairFrozen(IOracleRegistry::PairFrozen {})
    }
    pub const fn no_reports() -> Self {
        Self::NoReportsAvailable(IOracleRegistry::NoReportsAvailable {})
    }
    pub const fn all_expired() -> Self {
        Self::AllReportsExpired(IOracleRegistry::AllReportsExpired {})
    }
    pub const fn expiry_too_short() -> Self {
        Self::ExpiryTooShort(IOracleRegistry::ExpiryTooShort {})
    }
    pub const fn breaker_exceeded() -> Self {
        Self::BreakerThresholdExceeded(IOracleRegistry::BreakerThresholdExceeded {})
    }
}
