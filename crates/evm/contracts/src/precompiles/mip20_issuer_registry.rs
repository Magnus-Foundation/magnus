pub use IMIP20IssuerRegistry::{
    IMIP20IssuerRegistryErrors as MIP20IssuerRegistryError,
    IMIP20IssuerRegistryEvents as MIP20IssuerRegistryEvent,
};
use alloy_primitives::Address;

crate::sol! {
    #[derive(Debug, PartialEq, Eq)]
    #[sol(abi)]
    interface IMIP20IssuerRegistry {
        function isApprovedIssuer(string memory currency, address issuer) external view returns (bool);
        function getApprovedIssuers(string memory currency) external view returns (address[] memory);

        // Governance-gated by FeeManager.governanceAdmin (sender check).
        function addApprovedIssuer(string memory currency, address issuer) external;
        function removeApprovedIssuer(string memory currency, address issuer) external;

        event IssuerApproved(string currency, address indexed issuer);
        event IssuerRevoked(string currency, address indexed issuer);

        error IssuerNotApproved(address issuer, string currency);
        error IssuerAlreadyApproved(address issuer, string currency);
        error IssuerNotInAllowlist(address issuer, string currency);
        error CurrencyNotRegistered(string currency);
        error OnlyGovernanceAdmin(address caller);
    }
}

impl MIP20IssuerRegistryError {
    pub fn issuer_not_approved(issuer: Address, currency: alloc::string::String) -> Self {
        Self::IssuerNotApproved(IMIP20IssuerRegistry::IssuerNotApproved { issuer, currency })
    }

    pub fn issuer_already_approved(issuer: Address, currency: alloc::string::String) -> Self {
        Self::IssuerAlreadyApproved(IMIP20IssuerRegistry::IssuerAlreadyApproved {
            issuer,
            currency,
        })
    }

    pub fn issuer_not_in_allowlist(issuer: Address, currency: alloc::string::String) -> Self {
        Self::IssuerNotInAllowlist(IMIP20IssuerRegistry::IssuerNotInAllowlist { issuer, currency })
    }

    pub fn currency_not_registered(currency: alloc::string::String) -> Self {
        Self::CurrencyNotRegistered(IMIP20IssuerRegistry::CurrencyNotRegistered { currency })
    }

    pub const fn only_governance_admin(caller: Address) -> Self {
        Self::OnlyGovernanceAdmin(IMIP20IssuerRegistry::OnlyGovernanceAdmin { caller })
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
    fn only_governance_admin_constructor_preserves_field() {
        let caller = Address::repeat_byte(0x33);
        let err = MIP20IssuerRegistryError::only_governance_admin(caller);
        match err {
            MIP20IssuerRegistryError::OnlyGovernanceAdmin(inner) => {
                assert_eq!(inner.caller, caller);
            }
            _ => panic!("expected OnlyGovernanceAdmin variant"),
        }
    }

    #[test]
    fn issuer_registry_error_selectors_are_distinct() {
        let selectors = [
            IMIP20IssuerRegistry::IssuerNotApproved::SELECTOR,
            IMIP20IssuerRegistry::IssuerAlreadyApproved::SELECTOR,
            IMIP20IssuerRegistry::IssuerNotInAllowlist::SELECTOR,
            IMIP20IssuerRegistry::CurrencyNotRegistered::SELECTOR,
            IMIP20IssuerRegistry::OnlyGovernanceAdmin::SELECTOR,
        ];

        for i in 0..selectors.len() {
            for j in (i + 1)..selectors.len() {
                assert_ne!(selectors[i], selectors[j]);
            }
        }
    }
}
