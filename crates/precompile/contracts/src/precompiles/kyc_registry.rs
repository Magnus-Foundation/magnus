pub use IKYCRegistry::{
    IKYCRegistryErrors as KYCRegistryError,
    IKYCRegistryEvents as KYCRegistryEvent,
};

crate::sol! {
    #[derive(Debug, PartialEq, Eq)]
    #[sol(abi)]
    interface IKYCRegistry {
        // View functions
        function isVerified(address account) external view returns (bool);
        function getKYCLevel(address account) external view returns (uint8);
        function getKYCRecord(address account) external view returns (uint8 level, uint64 expiry, address verifier, uint8 jurisdiction);
        function isVerifier(address verifier) external view returns (bool);
        function owner() external view returns (address);

        // Verifier functions
        function setVerified(address account, uint8 level, uint64 expiry, uint8 jurisdiction) external;
        function revoke(address account) external;
        function batchSetVerified(address[] calldata accounts, uint8 level, uint64 expiry, uint8 jurisdiction) external;

        // Owner functions
        function addVerifier(address verifier) external;
        function removeVerifier(address verifier) external;
        function transferOwnership(address newOwner) external;

        // Events
        event KYCVerified(address indexed account, address indexed verifier, uint8 level, uint64 expiry, uint8 jurisdiction);
        event KYCRevoked(address indexed account, address indexed revoker);
        event VerifierAdded(address indexed verifier, address indexed addedBy);
        event VerifierRemoved(address indexed verifier, address indexed removedBy);
        event OwnershipTransferred(address indexed previousOwner, address indexed newOwner);

        // Errors
        error Unauthorized();
        error KYCNotFound();
        error InvalidLevel();
        error ExpiryInPast();
    }
}

impl KYCRegistryError {
    pub const fn unauthorized() -> Self {
        Self::Unauthorized(IKYCRegistry::Unauthorized {})
    }
    pub const fn kyc_not_found() -> Self {
        Self::KYCNotFound(IKYCRegistry::KYCNotFound {})
    }
    pub const fn invalid_level() -> Self {
        Self::InvalidLevel(IKYCRegistry::InvalidLevel {})
    }
    pub const fn expiry_in_past() -> Self {
        Self::ExpiryInPast(IKYCRegistry::ExpiryInPast {})
    }
}
