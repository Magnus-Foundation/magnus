pub use IMIP20IssuerRegistry::{
    IMIP20IssuerRegistryErrors as MIP20IssuerRegistryError,
    IMIP20IssuerRegistryEvents as MIP20IssuerRegistryEvent,
};
use alloy_primitives::Address;

crate::sol! {
    #[derive(Debug, PartialEq, Eq)]
    #[sol(abi)]
    interface IMIP20IssuerRegistry {
        // View Functions
        function isApprovedIssuer(string memory currency, address issuer) external view returns (bool);
        function getApprovedIssuers(string memory currency) external view returns (address[] memory);

        // State-Changing Functions (governance-gated)
        function addApprovedIssuer(
            string memory currency,
            address issuer,
            uint64 nonce,
            uint64 expiresAt,
            bytes calldata governanceSig
        ) external;

        function removeApprovedIssuer(
            string memory currency,
            address issuer,
            uint64 nonce,
            uint64 expiresAt,
            bytes calldata governanceSig
        ) external;

        // Events
        event IssuerApproved(string currency, address indexed issuer);
        event IssuerRevoked(string currency, address indexed issuer);

        // Errors
        error IssuerNotApproved(address issuer, string currency);
        error IssuerAlreadyApproved(address issuer, string currency);
        error CurrencyNotRegistered(string currency);
        error InvalidGovernanceSignature();
    }
}

impl MIP20IssuerRegistryError {
    /// Creates an error when the issuer is not approved for the given currency.
    pub fn issuer_not_approved(issuer: Address, currency: alloc::string::String) -> Self {
        Self::IssuerNotApproved(IMIP20IssuerRegistry::IssuerNotApproved { issuer, currency })
    }

    /// Creates an error when an issuer is already approved for the given currency.
    pub fn issuer_already_approved(issuer: Address, currency: alloc::string::String) -> Self {
        Self::IssuerAlreadyApproved(IMIP20IssuerRegistry::IssuerAlreadyApproved {
            issuer,
            currency,
        })
    }

    /// Creates an error when the currency is not in the FeeManager's currency registry.
    pub fn currency_not_registered(currency: alloc::string::String) -> Self {
        Self::CurrencyNotRegistered(IMIP20IssuerRegistry::CurrencyNotRegistered { currency })
    }

    /// Creates an error when the governance signature fails verification.
    pub const fn invalid_governance_signature() -> Self {
        Self::InvalidGovernanceSignature(IMIP20IssuerRegistry::InvalidGovernanceSignature {})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use alloy_sol_types::SolError;

    #[test]
    fn issuer_not_approved_constructor_preserves_fields() {
        let issuer = Address::repeat_byte(0x11);
        let err = MIP20IssuerRegistryError::issuer_not_approved(issuer, "USD".to_string());
        match err {
            MIP20IssuerRegistryError::IssuerNotApproved(inner) => {
                assert_eq!(inner.issuer, issuer);
                assert_eq!(inner.currency, "USD");
            }
            _ => panic!("expected IssuerNotApproved variant"),
        }
    }

    #[test]
    fn issuer_already_approved_constructor_preserves_fields() {
        let issuer = Address::repeat_byte(0x22);
        let err = MIP20IssuerRegistryError::issuer_already_approved(issuer, "VND".to_string());
        match err {
            MIP20IssuerRegistryError::IssuerAlreadyApproved(inner) => {
                assert_eq!(inner.issuer, issuer);
                assert_eq!(inner.currency, "VND");
            }
            _ => panic!("expected IssuerAlreadyApproved variant"),
        }
    }

    #[test]
    fn currency_not_registered_constructor_preserves_field() {
        let err = MIP20IssuerRegistryError::currency_not_registered("FOO".to_string());
        match err {
            MIP20IssuerRegistryError::CurrencyNotRegistered(inner) => {
                assert_eq!(inner.currency, "FOO");
            }
            _ => panic!("expected CurrencyNotRegistered variant"),
        }
    }

    #[test]
    fn invalid_governance_signature_is_unit_variant() {
        let err = MIP20IssuerRegistryError::invalid_governance_signature();
        assert!(matches!(
            err,
            MIP20IssuerRegistryError::InvalidGovernanceSignature(_)
        ));
    }

    /// All four error selectors on the new precompile must be unique. A
    /// collision would break ABI decoding from clients.
    #[test]
    fn issuer_registry_error_selectors_are_distinct() {
        let selectors = [
            IMIP20IssuerRegistry::IssuerNotApproved::SELECTOR,
            IMIP20IssuerRegistry::IssuerAlreadyApproved::SELECTOR,
            IMIP20IssuerRegistry::CurrencyNotRegistered::SELECTOR,
            IMIP20IssuerRegistry::InvalidGovernanceSignature::SELECTOR,
        ];

        for i in 0..selectors.len() {
            for j in (i + 1)..selectors.len() {
                assert_ne!(
                    selectors[i], selectors[j],
                    "issuer-registry error selectors {} and {} collide",
                    i, j
                );
            }
        }
    }
}
