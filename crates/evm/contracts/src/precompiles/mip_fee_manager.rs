pub use IFeeManager::{IFeeManagerErrors as FeeManagerError, IFeeManagerEvents as FeeManagerEvent};
pub use ITIPFeeAMM::{ITIPFeeAMMErrors as TIPFeeAMMError, ITIPFeeAMMEvents as TIPFeeAMMEvent};

crate::sol! {
    /// FeeManager interface for managing gas fee collection and distribution.
    ///
    /// IMPORTANT: FeeManager inherits from TIPFeeAMM and shares the same storage layout.
    /// This means:
    /// - FeeManager has all the functionality of TIPFeeAMM (pool management, swaps, liquidity operations)
    /// - Both contracts use the same storage slots for AMM data (pools, reserves, liquidity balances)
    /// - FeeManager extends TIPFeeAMM with additional storage slots (4-15) for fee-specific data
    /// - When deployed, FeeManager IS a TIPFeeAMM with additional fee management capabilities
    ///
    /// Storage layout:
    /// - Slots 0-3: TIPFeeAMM storage (pools, pool exists, liquidity data)
    /// - Slots 4+: FeeManager-specific storage (validator tokens, user tokens, collected fees, etc.)
    #[derive(Debug, PartialEq, Eq)]
    #[sol(abi)]
    interface IFeeManager {
        // Structs
        struct FeeInfo {
            uint128 amount;
            bool hasBeenSet;
        }

        /// `registered` is the existence flag — `false` for any never-added code
        /// (default-zero slot is indistinguishable from "added at block 0"). Off-chain
        /// readers MUST check `registered` before interpreting other fields.
        struct CurrencyConfig {
            bool   registered;
            bool   enabled;
            uint64 addedAtBlock;
            uint64 enabledAtBlock;
        }

        // User preferences
        function userTokens(address user) external view returns (address);
        function validatorTokens(address validator) external view returns (address);
        function setUserToken(address token) external;
        function setValidatorToken(address token) external;

        // Fee functions
        function distributeFees(address validator, address token) external;
        function collectedFees(address validator, address token) external view returns (uint256);
        // NOTE: collectFeePreTx is a protocol-internal function called directly by the
        // execution handler, not exposed via the dispatch interface.

        // Currency registry (governance-gated by `governanceAdmin`).
        function addCurrency(string calldata code) external;
        function enableCurrency(string calldata code) external;
        function setGovernanceAdmin(address newAdmin) external;
        function getCurrencyConfig(string calldata code) external view returns (CurrencyConfig memory);
        function isCurrencyEnabled(string calldata code) external view returns (bool);
        function governanceAdmin() external view returns (address);

        // Validator multi-token accept-set.
        function addAcceptedToken(address token) external;
        function removeAcceptedToken(address token) external;
        function acceptsToken(address validator, address token) external view returns (bool);
        function getAcceptedTokens(address validator) external view returns (address[] memory);
        function isAcceptedByAnyValidator(address token) external view returns (bool);

        // Events
        event UserTokenSet(address indexed user, address indexed token);
        event ValidatorTokenSet(address indexed validator, address indexed token);
        event FeesDistributed(address indexed validator, address indexed token, uint256 amount);
        event CurrencyAdded(string code, uint64 atBlock);
        event CurrencyEnabled(string code, uint64 atBlock);
        event GovernanceAdminChanged(address indexed oldAdmin, address indexed newAdmin);
        event AcceptedTokenAdded(address indexed validator, address indexed token);
        event AcceptedTokenRemoved(address indexed validator, address indexed token);

        // Errors
        error OnlyValidator();
        error OnlySystemContract();
        error InvalidToken();
        error PoolDoesNotExist();
        error InsufficientFeeTokenBalance();
        error InternalError();
        error CannotChangeWithinBlock();
        error CannotChangeWithPendingFees();
        error TokenPolicyForbids();

        error CurrencyNotRegistered(string currency);
        error CurrencyDisabled(string currency);
        error FeeTokenNotAccepted(address validator, address token);
        error FeeTokenNotInferable();
        error ValidatorAcceptSetEmpty(address validator);

        error OnlyGovernanceAdmin(address caller);
        error CurrencyAlreadyAdded(string currency);
        error CurrencyAlreadyEnabled(string currency);
        error InvalidCurrencyCode(string currency);
        error ZeroAddressGovernanceAdmin();

        error TokenAlreadyAccepted(address validator, address token);
        error TokenNotInAcceptSet(address validator, address token);
        error MaxAcceptSetReached(address validator);

        error UserTokenApiRemoved();
    }
}

sol! {
    /// TIPFeeAMM interface defining the base AMM functionality for stablecoin pools.
    /// This interface provides core liquidity pool management and swap operations.
    ///
    /// NOTE: The FeeManager contract inherits from TIPFeeAMM and shares the same storage layout.
    /// When FeeManager is deployed, it effectively "is" a TIPFeeAMM with additional fee management
    /// capabilities layered on top. Both contracts operate on the same storage slots.
    #[derive(Debug, PartialEq, Eq)]
    #[allow(clippy::too_many_arguments)]
    interface ITIPFeeAMM {
        // Structs
        struct Pool {
            uint128 reserveUserToken;
            uint128 reserveValidatorToken;
        }

        struct PoolKey {
            address token0;
            address token1;
        }


        // Constants
        function M() external view returns (uint256);
        function N() external view returns (uint256);
        function SCALE() external view returns (uint256);
        function MIN_LIQUIDITY() external view returns (uint256);

        // Pool Management
        function getPoolId(address userToken, address validatorToken) external pure returns (bytes32);
        function getPool(address userToken, address validatorToken) external view returns (Pool memory);
        function pools(bytes32 poolId) external view returns (Pool memory);

        // Liquidity Operations
        function mint(address userToken, address validatorToken, uint256 amountValidatorToken, address to) external returns (uint256 liquidity);
        function burn(address userToken, address validatorToken, uint256 liquidity, address to) external returns (uint256 amountUserToken, uint256 amountValidatorToken);

        // Liquidity Balances
        function totalSupply(bytes32 poolId) external view returns (uint256);
        function liquidityBalances(bytes32 poolId, address user) external view returns (uint256);

        // Swapping
        function rebalanceSwap(address userToken, address validatorToken, uint256 amountOut, address to) external returns (uint256 amountIn);

        // Events
        event Mint(address sender, address indexed to, address indexed userToken, address indexed validatorToken, uint256 amountValidatorToken, uint256 liquidity);
        event Burn(address indexed sender, address indexed userToken, address indexed validatorToken, uint256 amountUserToken, uint256 amountValidatorToken, uint256 liquidity, address to);
        event RebalanceSwap(address indexed userToken, address indexed validatorToken, address indexed swapper, uint256 amountIn, uint256 amountOut);

        // Errors
        error IdenticalAddresses();
        error InvalidToken();
        error InsufficientLiquidity();
        error InsufficientReserves();
        error InvalidAmount();
        error DivisionByZero();
        error InvalidSwapCalculation();
    }
}

impl FeeManagerError {
    /// Creates an error for only validator access.
    pub const fn only_validator() -> Self {
        Self::OnlyValidator(IFeeManager::OnlyValidator {})
    }

    /// Creates an error for only system contract access.
    pub const fn only_system_contract() -> Self {
        Self::OnlySystemContract(IFeeManager::OnlySystemContract {})
    }

    /// Creates an error for invalid token.
    pub const fn invalid_token() -> Self {
        Self::InvalidToken(IFeeManager::InvalidToken {})
    }

    /// Creates an error when pool does not exist.
    pub const fn pool_does_not_exist() -> Self {
        Self::PoolDoesNotExist(IFeeManager::PoolDoesNotExist {})
    }

    /// Creates an error for insufficient fee token balance.
    pub const fn insufficient_fee_token_balance() -> Self {
        Self::InsufficientFeeTokenBalance(IFeeManager::InsufficientFeeTokenBalance {})
    }

    /// Creates an error for cannot change within block.
    pub const fn cannot_change_within_block() -> Self {
        Self::CannotChangeWithinBlock(IFeeManager::CannotChangeWithinBlock {})
    }

    /// Creates an error for cannot change with pending fees.
    pub const fn cannot_change_with_pending_fees() -> Self {
        Self::CannotChangeWithPendingFees(IFeeManager::CannotChangeWithPendingFees {})
    }

    /// Creates an error for token policy forbids.
    pub const fn token_policy_forbids() -> Self {
        Self::TokenPolicyForbids(IFeeManager::TokenPolicyForbids {})
    }

    /// Creates an error for an unregistered currency.
    pub fn currency_not_registered(currency: alloc::string::String) -> Self {
        Self::CurrencyNotRegistered(IFeeManager::CurrencyNotRegistered { currency })
    }

    /// Creates an error for a disabled currency.
    pub fn currency_disabled(currency: alloc::string::String) -> Self {
        Self::CurrencyDisabled(IFeeManager::CurrencyDisabled { currency })
    }

    /// Creates an error when the validator's accept-set does not include `token`.
    pub fn fee_token_not_accepted(validator: alloy_primitives::Address, token: alloy_primitives::Address) -> Self {
        Self::FeeTokenNotAccepted(IFeeManager::FeeTokenNotAccepted { validator, token })
    }

    /// Creates an error when no fee token can be inferred from the tx calldata.
    pub const fn fee_token_not_inferable() -> Self {
        Self::FeeTokenNotInferable(IFeeManager::FeeTokenNotInferable {})
    }

    /// Creates an error when the producing validator has no tokens in its accept-set.
    pub fn validator_accept_set_empty(validator: alloy_primitives::Address) -> Self {
        Self::ValidatorAcceptSetEmpty(IFeeManager::ValidatorAcceptSetEmpty { validator })
    }

    pub fn only_governance_admin(caller: alloy_primitives::Address) -> Self {
        Self::OnlyGovernanceAdmin(IFeeManager::OnlyGovernanceAdmin { caller })
    }

    pub fn currency_already_added(currency: alloc::string::String) -> Self {
        Self::CurrencyAlreadyAdded(IFeeManager::CurrencyAlreadyAdded { currency })
    }

    pub fn currency_already_enabled(currency: alloc::string::String) -> Self {
        Self::CurrencyAlreadyEnabled(IFeeManager::CurrencyAlreadyEnabled { currency })
    }

    pub fn invalid_currency_code(currency: alloc::string::String) -> Self {
        Self::InvalidCurrencyCode(IFeeManager::InvalidCurrencyCode { currency })
    }

    pub const fn zero_address_governance_admin() -> Self {
        Self::ZeroAddressGovernanceAdmin(IFeeManager::ZeroAddressGovernanceAdmin {})
    }

    pub const fn token_already_accepted(
        validator: alloy_primitives::Address,
        token: alloy_primitives::Address,
    ) -> Self {
        Self::TokenAlreadyAccepted(IFeeManager::TokenAlreadyAccepted { validator, token })
    }

    pub const fn token_not_in_accept_set(
        validator: alloy_primitives::Address,
        token: alloy_primitives::Address,
    ) -> Self {
        Self::TokenNotInAcceptSet(IFeeManager::TokenNotInAcceptSet { validator, token })
    }

    pub const fn max_accept_set_reached(validator: alloy_primitives::Address) -> Self {
        Self::MaxAcceptSetReached(IFeeManager::MaxAcceptSetReached { validator })
    }

    pub const fn user_token_api_removed() -> Self {
        Self::UserTokenApiRemoved(IFeeManager::UserTokenApiRemoved {})
    }
}

#[cfg(test)]
mod fee_manager_error_tests {
    use super::*;
    use alloc::string::ToString;
    use alloy_primitives::Address;
    use alloy_sol_types::SolError;

    #[test]
    fn currency_not_registered_constructor_preserves_field() {
        let err = FeeManagerError::currency_not_registered("XYZ".to_string());
        match err {
            FeeManagerError::CurrencyNotRegistered(inner) => {
                assert_eq!(inner.currency, "XYZ");
            }
            _ => panic!("expected CurrencyNotRegistered variant"),
        }
    }

    #[test]
    fn currency_disabled_constructor_preserves_field() {
        let err = FeeManagerError::currency_disabled("USD".to_string());
        match err {
            FeeManagerError::CurrencyDisabled(inner) => {
                assert_eq!(inner.currency, "USD");
            }
            _ => panic!("expected CurrencyDisabled variant"),
        }
    }

    #[test]
    fn fee_token_not_accepted_constructor_preserves_fields() {
        let validator = Address::repeat_byte(0xAB);
        let token = Address::repeat_byte(0xCD);
        let err = FeeManagerError::fee_token_not_accepted(validator, token);

        match err {
            FeeManagerError::FeeTokenNotAccepted(inner) => {
                assert_eq!(inner.validator, validator);
                assert_eq!(inner.token, token);
            }
            _ => panic!("expected FeeTokenNotAccepted variant"),
        }
    }

    #[test]
    fn fee_token_not_inferable_is_unit_variant() {
        let err = FeeManagerError::fee_token_not_inferable();
        assert!(matches!(err, FeeManagerError::FeeTokenNotInferable(_)));
    }

    #[test]
    fn validator_accept_set_empty_constructor_preserves_field() {
        let validator = Address::repeat_byte(0x42);
        let err = FeeManagerError::validator_accept_set_empty(validator);
        match err {
            FeeManagerError::ValidatorAcceptSetEmpty(inner) => {
                assert_eq!(inner.validator, validator);
            }
            _ => panic!("expected ValidatorAcceptSetEmpty variant"),
        }
    }

    #[test]
    fn currency_error_selectors_are_distinct_from_each_other() {
        let new_selectors = [
            IFeeManager::CurrencyNotRegistered::SELECTOR,
            IFeeManager::CurrencyDisabled::SELECTOR,
            IFeeManager::FeeTokenNotAccepted::SELECTOR,
            IFeeManager::FeeTokenNotInferable::SELECTOR,
            IFeeManager::ValidatorAcceptSetEmpty::SELECTOR,
        ];

        for i in 0..new_selectors.len() {
            for j in (i + 1)..new_selectors.len() {
                assert_ne!(
                    new_selectors[i], new_selectors[j],
                    "currency error selectors {} and {} collide", i, j
                );
            }
        }
    }

    #[test]
    fn currency_error_selectors_are_distinct_from_existing_errors() {
        let new_selectors = [
            IFeeManager::CurrencyNotRegistered::SELECTOR,
            IFeeManager::CurrencyDisabled::SELECTOR,
            IFeeManager::FeeTokenNotAccepted::SELECTOR,
            IFeeManager::FeeTokenNotInferable::SELECTOR,
            IFeeManager::ValidatorAcceptSetEmpty::SELECTOR,
        ];
        let existing_selectors = [
            IFeeManager::OnlyValidator::SELECTOR,
            IFeeManager::OnlySystemContract::SELECTOR,
            IFeeManager::InvalidToken::SELECTOR,
            IFeeManager::PoolDoesNotExist::SELECTOR,
            IFeeManager::InsufficientFeeTokenBalance::SELECTOR,
            IFeeManager::InternalError::SELECTOR,
            IFeeManager::CannotChangeWithinBlock::SELECTOR,
            IFeeManager::CannotChangeWithPendingFees::SELECTOR,
            IFeeManager::TokenPolicyForbids::SELECTOR,
        ];

        for new in new_selectors {
            for existing in existing_selectors {
                assert_ne!(
                    new, existing,
                    "currency error selector {:?} collides with existing FeeManager error {:?}",
                    new, existing
                );
            }
        }
    }
}

impl TIPFeeAMMError {
    /// Creates an error for identical token addresses.
    pub const fn identical_addresses() -> Self {
        Self::IdenticalAddresses(ITIPFeeAMM::IdenticalAddresses {})
    }

    /// Creates an error for invalid token.
    pub const fn invalid_token() -> Self {
        Self::InvalidToken(ITIPFeeAMM::InvalidToken {})
    }

    /// Creates an error for insufficient liquidity.
    pub const fn insufficient_liquidity() -> Self {
        Self::InsufficientLiquidity(ITIPFeeAMM::InsufficientLiquidity {})
    }

    /// Creates an error for insufficient reserves.
    pub const fn insufficient_reserves() -> Self {
        Self::InsufficientReserves(ITIPFeeAMM::InsufficientReserves {})
    }
    /// Creates an error for invalid amount.
    pub const fn invalid_amount() -> Self {
        Self::InvalidAmount(ITIPFeeAMM::InvalidAmount {})
    }

    /// Creates an error for invalid swap calculation.
    pub const fn invalid_swap_calculation() -> Self {
        Self::InvalidSwapCalculation(ITIPFeeAMM::InvalidSwapCalculation {})
    }

    /// Creates an error for division by zero.
    pub const fn division_by_zero() -> Self {
        Self::DivisionByZero(ITIPFeeAMM::DivisionByZero {})
    }
}
