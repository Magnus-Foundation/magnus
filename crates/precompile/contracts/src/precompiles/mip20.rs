pub use IRolesAuth::{IRolesAuthErrors as RolesAuthError, IRolesAuthEvents as RolesAuthEvent};
pub use IMIP20::{IMIP20Errors as MIP20Error, IMIP20Events as MIP20Event};
use alloy_primitives::{Address, U256};

crate::sol! {
    #[derive(Debug, PartialEq, Eq)]
    #[sol(abi)]
    interface IRolesAuth {
        function hasRole(address account, bytes32 role) external view returns (bool);
        function getRoleAdmin(bytes32 role) external view returns (bytes32);
        function grantRole(bytes32 role, address account) external;
        function revokeRole(bytes32 role, address account) external;
        function renounceRole(bytes32 role) external;
        function setRoleAdmin(bytes32 role, bytes32 adminRole) external;

        event RoleMembershipUpdated(bytes32 indexed role, address indexed account, address indexed sender, bool hasRole);
        event RoleAdminUpdated(bytes32 indexed role, bytes32 indexed newAdminRole, address indexed sender);

        error Unauthorized();
    }
}

crate::sol! {
    /// MIP20 token interface providing standard ERC20 functionality with Magnus-specific extensions.
    ///
    /// MIP20 tokens extend the ERC20 standard with:
    /// - Currency denomination support for real-world asset backing
    /// - Transfer policy enforcement for compliance
    /// - Supply caps for controlled token issuance
    /// - Pause/unpause functionality for emergency controls
    /// - Memo support for transaction context
    /// The interface supports both standard token operations and administrative functions
    /// for managing token behavior and compliance requirements.
    #[derive(Debug, PartialEq, Eq)]
    #[sol(abi)]
    #[allow(clippy::too_many_arguments)]
    interface IMIP20 {
        // Standard token functions
        function name() external view returns (string memory);
        function symbol() external view returns (string memory);
        function decimals() external view returns (uint8);
        function totalSupply() external view returns (uint256);
        function quoteToken() external view returns (address);
        function nextQuoteToken() external view returns (address);
        function balanceOf(address account) external view returns (uint256);
        function transfer(address to, uint256 amount) external returns (bool);
        function approve(address spender, uint256 amount) external returns (bool);
        function allowance(address owner, address spender) external view returns (uint256);
        function transferFrom(address from, address to, uint256 amount) external returns (bool);
        function mint(address to, uint256 amount) external;
        function burn(uint256 amount) external;

        // MIP20 Extension
        function currency() external view returns (string memory);
        function supplyCap() external view returns (uint256);
        function paused() external view returns (bool);
        function transferPolicyId() external view returns (uint64);
        function burnBlocked(address from, uint256 amount) external;
        function mintWithMemo(address to, uint256 amount, bytes32 memo) external;
        function burnWithMemo(uint256 amount, bytes32 memo) external;
        function transferWithMemo(address to, uint256 amount, bytes32 memo) external;
        function transferFromWithMemo(address from, address to, uint256 amount, bytes32 memo) external returns (bool);

        // Admin Functions
        function changeTransferPolicyId(uint64 newPolicyId) external;
        function setSupplyCap(uint256 newSupplyCap) external;
        function pause() external;
        function unpause() external;
        function setNextQuoteToken(address newQuoteToken) external;
        function completeQuoteTokenUpdate() external;

        /// @notice Returns the role identifier for pausing the contract
        /// @return The pause role identifier
        function PAUSE_ROLE() external view returns (bytes32);

        /// @notice Returns the role identifier for unpausing the contract
        /// @return The unpause role identifier
        function UNPAUSE_ROLE() external view returns (bytes32);

        /// @notice Returns the role identifier for issuing tokens
        /// @return The issuer role identifier
        function ISSUER_ROLE() external view returns (bytes32);

        /// @notice Returns the role identifier for burning tokens from blocked accounts
        /// @return The burn blocked role identifier
        function BURN_BLOCKED_ROLE() external view returns (bytes32);

        struct UserRewardInfo {
            address rewardRecipient;
            uint256 rewardPerToken;
            uint256 rewardBalance;
        }

        // Reward Functions
        function distributeReward(uint256 amount) external;
        function setRewardRecipient(address recipient) external;
        function claimRewards() external returns (uint256);
        function optedInSupply() external view returns (uint128);
        function globalRewardPerToken() external view returns (uint256);
        function userRewardInfo(address account) external view returns (UserRewardInfo memory);
        function getPendingRewards(address account) external view returns (uint128);

        // Events
        event Transfer(address indexed from, address indexed to, uint256 amount);
        event Approval(address indexed owner, address indexed spender, uint256 amount);
        event Mint(address indexed to, uint256 amount);
        event Burn(address indexed from, uint256 amount);
        event BurnBlocked(address indexed from, uint256 amount);
        event TransferWithMemo(address indexed from, address indexed to, uint256 amount, bytes32 indexed memo);
        event TransferPolicyUpdate(address indexed updater, uint64 indexed newPolicyId);
        event SupplyCapUpdate(address indexed updater, uint256 indexed newSupplyCap);
        event PauseStateUpdate(address indexed updater, bool isPaused);
        event NextQuoteTokenSet(address indexed updater, address indexed nextQuoteToken);
        event QuoteTokenUpdate(address indexed updater, address indexed newQuoteToken);
        event RewardDistributed(address indexed funder, uint256 amount);
        event RewardRecipientSet(address indexed holder, address indexed recipient);

        // Errors
        error InsufficientBalance(uint256 available, uint256 required, address token);
        error InsufficientAllowance();
        error SupplyCapExceeded();
        error InvalidSupplyCap();
        error InvalidPayload();
        error StringTooLong();
        error PolicyForbids();
        error InvalidRecipient();
        error ContractPaused();
        error InvalidCurrency();
        error InvalidQuoteToken();
        error TransfersDisabled();
        error InvalidAmount();
        error NoOptedInSupply();
        error Unauthorized();
        error ProtectedAddress();
        error InvalidToken();
        error Uninitialized();
        error InvalidTransferPolicyId();
    }
}

impl RolesAuthError {
    /// Creates an error for unauthorized access.
    pub const fn unauthorized() -> Self {
        Self::Unauthorized(IRolesAuth::Unauthorized {})
    }
}

impl MIP20Error {
    /// Creates an error for insufficient token balance.
    pub const fn insufficient_balance(available: U256, required: U256, token: Address) -> Self {
        Self::InsufficientBalance(IMIP20::InsufficientBalance {
            available,
            required,
            token,
        })
    }

    /// Creates an error for insufficient spending allowance.
    pub const fn insufficient_allowance() -> Self {
        Self::InsufficientAllowance(IMIP20::InsufficientAllowance {})
    }

    /// Creates an error for unauthorized callers
    pub const fn unauthorized() -> Self {
        Self::Unauthorized(IMIP20::Unauthorized {})
    }

    /// Creates an error when minting would set a supply cap that is too large, or invalid.
    pub const fn invalid_supply_cap() -> Self {
        Self::InvalidSupplyCap(IMIP20::InvalidSupplyCap {})
    }

    /// Creates an error when minting would exceed supply cap.
    pub const fn supply_cap_exceeded() -> Self {
        Self::SupplyCapExceeded(IMIP20::SupplyCapExceeded {})
    }

    /// Creates an error for invalid payload data.
    pub const fn invalid_payload() -> Self {
        Self::InvalidPayload(IMIP20::InvalidPayload {})
    }

    /// Creates an error for invalid quote token.
    pub const fn invalid_quote_token() -> Self {
        Self::InvalidQuoteToken(IMIP20::InvalidQuoteToken {})
    }

    /// Creates an error when string parameter exceeds maximum length.
    pub const fn string_too_long() -> Self {
        Self::StringTooLong(IMIP20::StringTooLong {})
    }

    /// Creates an error when transfer is forbidden by policy.
    pub const fn policy_forbids() -> Self {
        Self::PolicyForbids(IMIP20::PolicyForbids {})
    }

    /// Creates an error for invalid recipient address.
    pub const fn invalid_recipient() -> Self {
        Self::InvalidRecipient(IMIP20::InvalidRecipient {})
    }

    /// Creates an error when contract is paused.
    pub const fn contract_paused() -> Self {
        Self::ContractPaused(IMIP20::ContractPaused {})
    }

    /// Creates an error for invalid currency.
    pub const fn invalid_currency() -> Self {
        Self::InvalidCurrency(IMIP20::InvalidCurrency {})
    }

    /// Creates an error for transfers being disabled.
    pub const fn transfers_disabled() -> Self {
        Self::TransfersDisabled(IMIP20::TransfersDisabled {})
    }

    /// Creates an error for invalid amount.
    pub const fn invalid_amount() -> Self {
        Self::InvalidAmount(IMIP20::InvalidAmount {})
    }

    /// Error for when opted in supply is 0
    pub const fn no_opted_in_supply() -> Self {
        Self::NoOptedInSupply(IMIP20::NoOptedInSupply {})
    }

    /// Error for operations on protected addresses (like burning `FeeManager` tokens)
    pub const fn protected_address() -> Self {
        Self::ProtectedAddress(IMIP20::ProtectedAddress {})
    }

    /// Error when an address is not a valid MIP20 token
    pub const fn invalid_token() -> Self {
        Self::InvalidToken(IMIP20::InvalidToken {})
    }

    /// Error when transfer policy ID does not exist
    pub const fn invalid_transfer_policy_id() -> Self {
        Self::InvalidTransferPolicyId(IMIP20::InvalidTransferPolicyId {})
    }

    /// Error when token is uninitialized (has no bytecode)
    pub const fn uninitialized() -> Self {
        Self::Uninitialized(IMIP20::Uninitialized {})
    }
}
