//! [Fee manager] precompile for transaction fee collection, distribution, and token swaps.
//!
//! [Fee manager]: <https://docs.magnus.xyz/protocol/fees>

pub mod amm;
pub mod currency_registry;
pub mod dispatch;

use crate::{
    error::{MagnusPrecompileError, Result},
    mip_fee_manager::amm::{Pool, PoolKey, compute_amount_out},
    mip_fee_manager::currency_registry::{CurrencyConfig, currency_key, is_valid_currency_code},
    mip20::{IMIP20, MIP20Token, validate_usd_currency},
    mip20_factory::MIP20Factory,
    storage::{Handler, Mapping},
};
use alloy::primitives::{Address, B256, U256, uint};
pub use magnus_contracts::precompiles::{
    DEFAULT_FEE_TOKEN, FeeManagerError, FeeManagerEvent, IFeeManager, ITIPFeeAMM,
    TIP_FEE_MANAGER_ADDRESS, TIPFeeAMMError, TIPFeeAMMEvent,
};
use magnus_precompiles_macros::contract;

/// Fee manager precompile that handles transaction fee collection and distribution.
///
/// Users and validators choose their preferred MIP-20 fee token. When they differ, fees are
/// swapped through the built-in AMM (`TIPFeeAMM`).
///
/// The struct fields define the on-chain storage layout; the `#[contract]` macro generates the
/// storage handlers which provide an ergonomic way to interact with the EVM state.
#[contract(addr = TIP_FEE_MANAGER_ADDRESS)]
pub struct MipFeeManager {
    validator_tokens: Mapping<Address, Address>,
    user_tokens: Mapping<Address, Address>,
    collected_fees: Mapping<Address, Mapping<Address, U256>>,
    pools: Mapping<B256, Pool>,
    total_supply: Mapping<B256, U256>,
    liquidity_balances: Mapping<B256, Mapping<Address, U256>>,

    // ─── G1: Currency registry (multi-currency-fees-design.md §4) ──────────────
    //
    // Authentication for governance-gated functions is `sender == governance_admin`.
    // The admin is intended to be the address of an off-chain or on-chain multisig
    // contract; signature aggregation happens at that layer, the precompile only
    // checks `msg.sender`. Future shared-infrastructure groups may upgrade this to
    // EIP-712 multisig verification embedded directly here, at which point this
    // field would be replaced with a signer-set + threshold pair.
    governance_admin: Address,
    /// Per-currency configuration map. Key = `keccak256(ISO 4217 code)` (B256).
    /// Solidity ABI takes the human-readable code; the precompile hashes internally.
    supported_currencies: Mapping<B256, CurrencyConfig>,

    // ─── G2a: Validator multi-token accept-set (design §6, §7) ────────────────
    //
    // Replaces the single-token `validator_tokens` model. Each validator-org chooses a
    // SET of tokens it will accept as fee payout. G2a only adds the new storage and
    // API; the legacy `validator_tokens` field stays in place for backward compatibility
    // until G2b/G3 rewire the fee-collection path.
    //
    // Storage layout:
    //   `validator_accepted_tokens[validator][token] -> bool`  membership flag
    //   `validator_token_list[validator] -> Vec<Address>`       enumeration order
    //
    // Both must stay in sync. Insertion appends to the list and flips the flag;
    // removal flips the flag and swap-removes from the list. The list exists so
    // off-chain readers can enumerate without scanning every (validator, token) pair.
    validator_accepted_tokens: Mapping<Address, Mapping<Address, bool>>,
    validator_token_list: Mapping<Address, Vec<Address>>,

    // WARNING(rusowsky): transient storage slots must always be placed at the very end until the `contract`
    // macro is refactored and has 2 independent layouts (persistent and transient).
    // If new (persistent) storage fields need to be added to the precompile, they must go above this one.
    /// T1C+: Tracks liquidity reserved for a pending fee swap during `collect_fee_pre_tx`.
    /// Checked by `burn` and `rebalance_swap` to prevent withdrawals that would violate the reservation.
    pending_fee_swap_reservation: Mapping<B256, u128>,
}

impl MipFeeManager {
    /// Swap fee in basis points (0.25%).
    pub const FEE_BPS: u64 = 25;
    /// Basis-point denominator (10 000 = 100%).
    pub const BASIS_POINTS: u64 = 10000;
    /// Minimum MIP-20 balance required for fee operations (1e9).
    pub const MINIMUM_BALANCE: U256 = uint!(1_000_000_000_U256);

    /// G2a: Maximum number of tokens a single validator may accept.
    /// Caps state bloat from a misbehaving validator that calls `addAcceptedToken`
    /// repeatedly. 32 is comfortably more than the realistic mainnet count
    /// (likely 5-10 currencies × 2-3 issuers each = 10-30 tokens).
    pub const MAX_ACCEPT_SET_SIZE: usize = 32;

    /// Initializes the fee manager precompile.
    pub fn initialize(&mut self) -> Result<()> {
        self.__initialize()
    }

    /// Returns the validator's preferred fee token, falling back to [`DEFAULT_FEE_TOKEN`].
    pub fn get_validator_token(&self, beneficiary: Address) -> Result<Address> {
        let token = self.validator_tokens[beneficiary].read()?;

        if token.is_zero() {
            Ok(DEFAULT_FEE_TOKEN)
        } else {
            Ok(token)
        }
    }

    /// Sets the caller's preferred fee token as a validator.
    ///
    /// Rejects the call if `sender` is the current block's beneficiary (prevents mid-block
    /// fee-token changes) or if the token is not a valid USD-denominated MIP-20 registered in
    /// [`MIP20Factory`].
    ///
    /// # Errors
    /// - `InvalidToken` — token is not a deployed MIP-20 in [`MIP20Factory`]
    /// - `CannotChangeWithinBlock` — `sender` equals the current block `beneficiary`
    /// - `InvalidCurrency` — token is not USD-denominated
    pub fn set_validator_token(
        &mut self,
        sender: Address,
        call: IFeeManager::setValidatorTokenCall,
        beneficiary: Address,
    ) -> Result<()> {
        // Validate that the token is a valid deployed MIP20
        if !MIP20Factory::new().is_tip20(call.token)? {
            return Err(FeeManagerError::invalid_token().into());
        }

        // Prevent changing within the validator's own block
        if sender == beneficiary {
            return Err(FeeManagerError::cannot_change_within_block().into());
        }

        // Validate that the fee token is USD
        validate_usd_currency(call.token)?;

        self.validator_tokens[sender].write(call.token)?;

        // Emit ValidatorTokenSet event
        self.emit_event(FeeManagerEvent::ValidatorTokenSet(
            IFeeManager::ValidatorTokenSet {
                validator: sender,
                token: call.token,
            },
        ))
    }

    /// Sets the caller's preferred fee token as a user. Must be a valid USD-denominated MIP-20
    /// registered in [`MIP20Factory`].
    ///
    /// # Errors
    /// - `InvalidToken` — token is not a deployed MIP-20 in [`MIP20Factory`]
    /// - `InvalidCurrency` — token is not USD-denominated
    pub fn set_user_token(
        &mut self,
        sender: Address,
        call: IFeeManager::setUserTokenCall,
    ) -> Result<()> {
        // Validate that the token is a valid deployed MIP20
        if !MIP20Factory::new().is_tip20(call.token)? {
            return Err(FeeManagerError::invalid_token().into());
        }

        // Validate that the fee token is USD
        validate_usd_currency(call.token)?;

        // T3+: skip write and event if the token is already set to the requested value.
        // Prevents permissionless callers from forcing redundant pool invalidation scans.
        if self.storage.spec().is_t3() {
            let current = self.user_tokens[sender].read()?;
            if current == call.token {
                return Ok(());
            }
        }

        self.user_tokens[sender].write(call.token)?;

        // Emit UserTokenSet event
        self.emit_event(FeeManagerEvent::UserTokenSet(IFeeManager::UserTokenSet {
            user: sender,
            token: call.token,
        }))
    }

    /// Collects fees from `fee_payer` before transaction execution.
    ///
    /// Transfers `max_amount` of `user_token` to the fee manager via [`MIP20Token`] and, if the
    /// validator prefers a different token, verifies sufficient pool liquidity
    /// (reserving it on T1C+). Returns the user's fee token.
    ///
    /// # Errors
    /// - `InvalidToken` — `user_token` does not have a valid MIP-20 prefix
    /// - `PolicyForbids` — MIP-403 policy rejects the fee token transfer
    /// - `InsufficientLiquidity` — AMM pool lacks liquidity for the fee swap
    pub fn collect_fee_pre_tx(
        &mut self,
        fee_payer: Address,
        user_token: Address,
        max_amount: U256,
        beneficiary: Address,
        skip_liquidity_check: bool,
    ) -> Result<Address> {
        // Get the validator's token preference
        let validator_token = self.get_validator_token(beneficiary)?;

        let mut mip20_token = MIP20Token::from_address(user_token)?;

        // Ensure that user and FeeManager are authorized to interact with the token
        mip20_token.ensure_transfer_authorized(fee_payer, self.address)?;
        mip20_token.transfer_fee_pre_tx(fee_payer, max_amount)?;

        if user_token != validator_token && !skip_liquidity_check {
            let pool_id = PoolKey::new(user_token, validator_token).get_id();
            let amount_out_needed = self.check_sufficient_liquidity(pool_id, max_amount)?;

            if self.storage.spec().is_t1c() {
                self.reserve_pool_liquidity(pool_id, amount_out_needed)?;
            }
        }

        // Return the user's token preference
        Ok(user_token)
    }

    /// Finalizes fee collection after transaction execution.
    ///
    /// Refunds unused `user_token` to `fee_payer` via [`MIP20Token`], executes the fee swap
    /// through the AMM pool if tokens differ, and accumulates fees for the validator.
    ///
    /// # Errors
    /// - `InvalidToken` — `fee_token` does not have a valid MIP-20 prefix
    /// - `InsufficientLiquidity` — AMM pool lacks liquidity for the fee swap
    /// - `UnderOverflow` — collected-fee accumulator overflows
    pub fn collect_fee_post_tx(
        &mut self,
        fee_payer: Address,
        actual_spending: U256,
        refund_amount: U256,
        fee_token: Address,
        beneficiary: Address,
    ) -> Result<()> {
        // Refund unused tokens to user
        let mut mip20_token = MIP20Token::from_address(fee_token)?;
        mip20_token.transfer_fee_post_tx(fee_payer, refund_amount, actual_spending)?;

        // Execute fee swap and track collected fees
        let validator_token = self.get_validator_token(beneficiary)?;

        if fee_token != validator_token && !actual_spending.is_zero() {
            // Execute fee swap immediately and accumulate fees
            self.execute_fee_swap(fee_token, validator_token, actual_spending)?;
        }

        let amount = if fee_token == validator_token {
            actual_spending
        } else {
            compute_amount_out(actual_spending)?
        };

        self.increment_collected_fees(beneficiary, validator_token, amount)?;

        Ok(())
    }

    /// Increment collected fees for a specific validator and token combination.
    fn increment_collected_fees(
        &mut self,
        validator: Address,
        token: Address,
        amount: U256,
    ) -> Result<()> {
        if amount.is_zero() {
            return Ok(());
        }

        let collected_fees = self.collected_fees[validator][token].read()?;
        self.collected_fees[validator][token].write(
            collected_fees
                .checked_add(amount)
                .ok_or(MagnusPrecompileError::under_overflow())?,
        )?;

        Ok(())
    }

    /// Transfers a validator's accumulated fee balance to their address via [`MIP20Token`] and
    /// zeroes the ledger. No-ops when the balance is zero.
    ///
    /// # Errors
    /// - `InvalidToken` — `token` does not have a valid MIP-20 prefix
    pub fn distribute_fees(&mut self, validator: Address, token: Address) -> Result<()> {
        let amount = self.collected_fees[validator][token].read()?;
        if amount.is_zero() {
            return Ok(());
        }
        self.collected_fees[validator][token].write(U256::ZERO)?;

        // Transfer fees to validator
        let mut mip20_token = MIP20Token::from_address(token)?;
        mip20_token.transfer(
            self.address,
            IMIP20::transferCall {
                to: validator,
                amount,
            },
        )?;

        // Emit FeesDistributed event
        self.emit_event(FeeManagerEvent::FeesDistributed(
            IFeeManager::FeesDistributed {
                validator,
                token,
                amount,
            },
        ))?;

        Ok(())
    }

    /// Reads the stored fee token preference for a user.
    pub fn user_tokens(&self, call: IFeeManager::userTokensCall) -> Result<Address> {
        self.user_tokens[call.user].read()
    }

    // ─── G1: Currency registry (multi-currency-fees-design.md §4) ──────────────
    //
    // Governance-gated setters: `addCurrency`, `enableCurrency`, `setGovernanceAdmin`.
    // Authentication is `sender == governance_admin`. Reads are public.

    /// Returns the address authorized to call governance-gated setters.
    /// Zero address means "not yet configured" — all governance setters revert until
    /// genesis or a one-time bootstrap call sets it.
    pub fn governance_admin(&self) -> Result<Address> {
        self.governance_admin.read()
    }

    /// Reads the on-chain config for a registered currency.
    /// Returns `CurrencyConfig::default()` (`enabled = false`, blocks = 0) for codes that
    /// have never been registered. Use `is_currency_enabled` for the explicit check.
    pub fn get_currency_config(&self, code: &str) -> Result<CurrencyConfig> {
        self.supported_currencies[currency_key(code)].read()
    }

    /// Returns `true` if `code` is registered AND currently enabled (gas-eligible).
    pub fn is_currency_enabled(&self, code: &str) -> Result<bool> {
        let config = self.get_currency_config(code)?;
        Ok(config.enabled)
    }

    /// Registers a new currency (initially disabled) — must be `enable`d separately.
    /// Two-step add → enable lets governance pre-stage a currency without making it
    /// gas-eligible until the issuer + validator-org coordination is done.
    ///
    /// # Errors
    /// - `OnlyGovernanceAdmin` — caller is not `governance_admin`
    /// - `InvalidCurrencyCode` — code fails ISO-4217 syntax check
    /// - `CurrencyAlreadyAdded` — code is already in the registry
    pub fn add_currency(&mut self, sender: Address, code: &str, current_block: u64) -> Result<()> {
        self.assert_governance(sender)?;
        if !is_valid_currency_code(code) {
            return Err(FeeManagerError::invalid_currency_code(code.into()).into());
        }

        let key = currency_key(code);
        let existing = self.supported_currencies[key].read()?;
        if existing.registered {
            return Err(FeeManagerError::currency_already_added(code.into()).into());
        }

        let config = CurrencyConfig::newly_added(current_block);
        self.supported_currencies[key].write(config)?;

        self.emit_event(FeeManagerEvent::CurrencyAdded(IFeeManager::CurrencyAdded {
            code: code.into(),
            atBlock: current_block,
        }))?;

        Ok(())
    }

    /// Marks a registered currency as gas-eligible. Idempotent: re-enable on an
    /// already-enabled currency reverts with `CurrencyAlreadyEnabled`.
    ///
    /// # Errors
    /// - `OnlyGovernanceAdmin` — caller is not `governance_admin`
    /// - `InvalidCurrencyCode` — code fails ISO-4217 syntax check
    /// - `CurrencyNotRegistered` — code has not been added via `addCurrency`
    /// - `CurrencyAlreadyEnabled` — code is already enabled
    pub fn enable_currency(
        &mut self,
        sender: Address,
        code: &str,
        current_block: u64,
    ) -> Result<()> {
        self.assert_governance(sender)?;
        if !is_valid_currency_code(code) {
            return Err(FeeManagerError::invalid_currency_code(code.into()).into());
        }

        let key = currency_key(code);
        let mut config = self.supported_currencies[key].read()?;
        if !config.registered {
            return Err(FeeManagerError::currency_not_registered(code.into()).into());
        }
        if config.enabled {
            return Err(FeeManagerError::currency_already_enabled(code.into()).into());
        }

        config.enabled = true;
        config.enabled_at_block = current_block;
        self.supported_currencies[key].write(config)?;

        self.emit_event(FeeManagerEvent::CurrencyEnabled(IFeeManager::CurrencyEnabled {
            code: code.into(),
            atBlock: current_block,
        }))?;

        Ok(())
    }

    /// Transfers governance authority to a new admin address. Only the current admin
    /// (or zero-address sentinel during genesis bootstrap) may call.
    ///
    /// # Errors
    /// - `OnlyGovernanceAdmin` — caller is not the current admin
    /// - `ZeroAddressGovernanceAdmin` — `new_admin == address(0)` would brick the registry
    pub fn set_governance_admin(&mut self, sender: Address, new_admin: Address) -> Result<()> {
        if new_admin.is_zero() {
            return Err(FeeManagerError::zero_address_governance_admin().into());
        }
        let old_admin = self.governance_admin.read()?;
        // Bootstrap rule: while admin is zero, anyone may set the initial admin (e.g. via the
        // genesis-init transaction or a privileged setup call). After that, only the current
        // admin can rotate authority.
        if !old_admin.is_zero() && sender != old_admin {
            return Err(FeeManagerError::only_governance_admin(sender).into());
        }
        self.governance_admin.write(new_admin)?;

        self.emit_event(FeeManagerEvent::GovernanceAdminChanged(
            IFeeManager::GovernanceAdminChanged {
                oldAdmin: old_admin,
                newAdmin: new_admin,
            },
        ))?;

        Ok(())
    }

    /// Internal helper: enforces `sender == governance_admin` for governance-gated setters.
    fn assert_governance(&self, sender: Address) -> Result<()> {
        let admin = self.governance_admin.read()?;
        if sender != admin {
            return Err(FeeManagerError::only_governance_admin(sender).into());
        }
        Ok(())
    }

    // ─── G2a: Validator multi-token accept-set API (design §6) ────────────────
    //
    // The legacy single-token `set_validator_token` / `get_validator_token` API stays
    // intact in this commit; G2b removes it and rewires the fee-collection path to
    // call `accepts_token` instead.

    /// Returns whether `validator` has added `token` to its accept-set.
    /// Returns `false` for any validator that has never called `add_accepted_token`.
    pub fn accepts_token(&self, validator: Address, token: Address) -> Result<bool> {
        self.validator_accepted_tokens[validator][token].read()
    }

    /// Returns the list of tokens `validator` accepts as fee payout.
    /// Empty for any validator that has never called `add_accepted_token`.
    pub fn get_accepted_tokens(&self, validator: Address) -> Result<Vec<Address>> {
        self.validator_token_list[validator].read()
    }

    /// Returns `true` if at least one validator in storage accepts `token`. The naive
    /// implementation here would require scanning every validator; the cheap
    /// G2a-compatible answer is "we don't know without an off-chain index". For now
    /// this is a placeholder that the wallet SDK queries via off-chain state-trie
    /// scanning; G4+ may add an explicit `accepted_token_validator_count[token]`
    /// counter mapping to make the answer cheap on-chain.
    ///
    /// **G2a stub:** always returns `false`. A wallet that needs the answer must
    /// query each validator individually via `accepts_token`. G2b/G4 may add the
    /// reverse-index mapping if real-world UX shows the on-chain answer is needed.
    pub fn is_accepted_by_any_validator(&self, _token: Address) -> Result<bool> {
        Ok(false)
    }

    /// Adds `token` to the caller's (validator's) accept-set.
    ///
    /// # Errors
    /// - `InvalidToken` — `token` is not a deployed MIP-20
    /// - `CurrencyNotRegistered` / `CurrencyDisabled` — token's currency is not gas-eligible
    /// - `CannotChangeWithinBlock` — caller is the current block's beneficiary
    /// - `MaxAcceptSetReached` — caller's accept-set already holds `MAX_ACCEPT_SET_SIZE` tokens
    /// - `TokenAlreadyAccepted` — `token` is already in the caller's accept-set
    pub fn add_accepted_token(
        &mut self,
        sender: Address,
        token: Address,
        beneficiary: Address,
    ) -> Result<()> {
        // Token validity: must be a deployed MIP-20 with a registered+enabled currency.
        if !MIP20Factory::new().is_tip20(token)? {
            return Err(FeeManagerError::invalid_token().into());
        }
        crate::mip20::validate_supported_currency(token)?;

        // Same-block-as-beneficiary protection: a validator producing the current block
        // cannot mutate its own accept-set in that block — it would change fee
        // semantics mid-block and create a foot-gun for downstream caching.
        if sender == beneficiary {
            return Err(FeeManagerError::cannot_change_within_block().into());
        }

        // Idempotency check.
        if self.validator_accepted_tokens[sender][token].read()? {
            return Err(FeeManagerError::token_already_accepted(sender, token).into());
        }

        // Cap on accept-set size.
        let mut list = self.validator_token_list[sender].read()?;
        if list.len() >= Self::MAX_ACCEPT_SET_SIZE {
            return Err(FeeManagerError::max_accept_set_reached(sender).into());
        }

        list.push(token);
        self.validator_token_list[sender].write(list)?;
        self.validator_accepted_tokens[sender][token].write(true)?;

        self.emit_event(FeeManagerEvent::AcceptedTokenAdded(
            IFeeManager::AcceptedTokenAdded {
                validator: sender,
                token,
            },
        ))?;

        Ok(())
    }

    /// Removes `token` from the caller's (validator's) accept-set.
    ///
    /// # Errors
    /// - `CannotChangeWithinBlock` — caller is the current block's beneficiary
    /// - `TokenNotInAcceptSet` — `token` is not in the caller's accept-set
    pub fn remove_accepted_token(
        &mut self,
        sender: Address,
        token: Address,
        beneficiary: Address,
    ) -> Result<()> {
        if sender == beneficiary {
            return Err(FeeManagerError::cannot_change_within_block().into());
        }

        if !self.validator_accepted_tokens[sender][token].read()? {
            return Err(FeeManagerError::token_not_in_accept_set(sender, token).into());
        }

        // Swap-remove from the list to keep it dense; order is irrelevant to consumers.
        let mut list = self.validator_token_list[sender].read()?;
        let pos = list.iter().position(|t| *t == token).ok_or_else(|| {
            // Storage inconsistency: flag was set but list doesn't contain the token.
            // This should be unreachable; treat it as a fatal error rather than a
            // silent cleanup so the underlying bug is visible.
            MagnusPrecompileError::Fatal(
                "validator_accepted_tokens flag set but token missing from list".into(),
            )
        })?;
        list.swap_remove(pos);
        self.validator_token_list[sender].write(list)?;
        self.validator_accepted_tokens[sender][token].write(false)?;

        self.emit_event(FeeManagerEvent::AcceptedTokenRemoved(
            IFeeManager::AcceptedTokenRemoved {
                validator: sender,
                token,
            },
        ))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use magnus_chainspec::hardfork::MagnusHardfork;
    use magnus_contracts::precompiles::MIP20Error;

    use super::*;
    use crate::{
        TIP_FEE_MANAGER_ADDRESS,
        error::MagnusPrecompileError,
        mip20::{IMIP20, MIP20Token},
        storage::{ContractStorage, StorageCtx, hashmap::HashMapStorageProvider},
        test_util::MIP20Setup,
    };

    #[test]
    fn test_set_user_token() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let user = Address::random();
        StorageCtx::enter(&mut storage, || {
            let token = MIP20Setup::create("Test", "TST", user).apply()?;

            // TODO: loop through and deploy and set user token for some range

            let mut fee_manager = MipFeeManager::new();

            let call = IFeeManager::setUserTokenCall {
                token: token.address(),
            };
            let result = fee_manager.set_user_token(user, call);
            assert!(result.is_ok());

            let call = IFeeManager::userTokensCall { user };
            assert_eq!(fee_manager.user_tokens(call)?, token.address());

            Ok(())
        })
    }

    #[test]
    fn test_set_user_token_noop_when_unchanged_pre_t3() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new_with_spec(1, MagnusHardfork::T2);
        let user = Address::random();
        StorageCtx::enter(&mut storage, || {
            let token = MIP20Setup::create("Test", "TST", user).apply()?;
            let mut fee_manager = MipFeeManager::new();

            let call = IFeeManager::setUserTokenCall {
                token: token.address(),
            };

            fee_manager.set_user_token(user, call.clone())?;
            fee_manager.set_user_token(user, call)?;
            let event_count = StorageCtx.get_events(TIP_FEE_MANAGER_ADDRESS).len();
            assert_eq!(
                event_count, 2,
                "pre-T3: event emitted even when token unchanged"
            );

            Ok(())
        })
    }

    #[test]
    fn test_set_user_token_noop_when_unchanged_t3() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new_with_spec(1, MagnusHardfork::T3);
        let user = Address::random();
        StorageCtx::enter(&mut storage, || {
            let token = MIP20Setup::create("Test", "TST", user).apply()?;
            let mut fee_manager = MipFeeManager::new();

            let call = IFeeManager::setUserTokenCall {
                token: token.address(),
            };

            fee_manager.set_user_token(user, call.clone())?;
            let event_count = StorageCtx.get_events(TIP_FEE_MANAGER_ADDRESS).len();
            assert_eq!(event_count, 1, "first set_user_token should emit event");

            fee_manager.set_user_token(user, call)?;
            let event_count = StorageCtx.get_events(TIP_FEE_MANAGER_ADDRESS).len();
            assert_eq!(
                event_count, 1,
                "T3+: repeated set_user_token with same token should not emit event"
            );

            Ok(())
        })
    }

    #[test]
    fn test_set_validator_token() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let validator = Address::random();
        let admin = Address::random();
        let beneficiary = Address::random();
        StorageCtx::enter(&mut storage, || {
            let token = MIP20Setup::create("Test", "TST", admin).apply()?;
            let mut fee_manager = MipFeeManager::new();

            let call = IFeeManager::setValidatorTokenCall {
                token: token.address(),
            };

            // Should fail when validator == beneficiary (same block check)
            let result = fee_manager.set_validator_token(validator, call.clone(), validator);
            assert_eq!(
                result,
                Err(MagnusPrecompileError::FeeManagerError(
                    FeeManagerError::cannot_change_within_block()
                ))
            );

            // Should succeed with different beneficiary
            let result = fee_manager.set_validator_token(validator, call, beneficiary);
            assert!(result.is_ok());

            let returned_token = fee_manager.get_validator_token(validator)?;
            assert_eq!(returned_token, token.address());

            Ok(())
        })
    }

    #[test]
    fn test_set_validator_token_cannot_change_within_block() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let validator = Address::random();
        let beneficiary = Address::random();
        let admin = Address::random();
        StorageCtx::enter(&mut storage, || {
            let token = MIP20Setup::create("Test", "TST", admin).apply()?;
            let mut fee_manager = MipFeeManager::new();

            let call = IFeeManager::setValidatorTokenCall {
                token: token.address(),
            };

            // Setting validator token when not beneficiary should succeed
            let result = fee_manager.set_validator_token(validator, call.clone(), beneficiary);
            assert!(result.is_ok());

            // But if validator is the beneficiary, should fail with CannotChangeWithinBlock
            let result = fee_manager.set_validator_token(validator, call, validator);
            assert_eq!(
                result,
                Err(MagnusPrecompileError::FeeManagerError(
                    FeeManagerError::cannot_change_within_block()
                ))
            );

            Ok(())
        })
    }

    #[test]
    fn test_collect_fee_pre_tx() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let user = Address::random();
        let validator = Address::random();
        let beneficiary = Address::random();
        StorageCtx::enter(&mut storage, || {
            let max_amount = U256::from(10000);

            let token = MIP20Setup::create("Test", "TST", user)
                .with_issuer(user)
                .with_mint(user, U256::from(u64::MAX))
                .with_approval(user, TIP_FEE_MANAGER_ADDRESS, U256::MAX)
                .apply()?;

            let mut fee_manager = MipFeeManager::new();

            // Set validator token (use beneficiary to avoid CannotChangeWithinBlock)
            fee_manager.set_validator_token(
                validator,
                IFeeManager::setValidatorTokenCall {
                    token: token.address(),
                },
                beneficiary,
            )?;

            // Set user token
            fee_manager.set_user_token(
                user,
                IFeeManager::setUserTokenCall {
                    token: token.address(),
                },
            )?;

            // Call collect_fee_pre_tx directly
            let result =
                fee_manager.collect_fee_pre_tx(user, token.address(), max_amount, validator, false);
            assert!(result.is_ok());
            assert_eq!(result?, token.address());

            Ok(())
        })
    }

    #[test]
    fn test_collect_fee_post_tx() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let user = Address::random();
        let admin = Address::random();
        let validator = Address::random();
        let beneficiary = Address::random();
        StorageCtx::enter(&mut storage, || {
            let actual_used = U256::from(6000);
            let refund_amount = U256::from(4000);

            // Mint to FeeManager (simulating collect_fee_pre_tx already happened)
            let token = MIP20Setup::create("Test", "TST", admin)
                .with_issuer(admin)
                .with_mint(TIP_FEE_MANAGER_ADDRESS, U256::from(100000000000000_u64))
                .apply()?;

            let mut fee_manager = MipFeeManager::new();

            // Set validator token (use beneficiary to avoid CannotChangeWithinBlock)
            fee_manager.set_validator_token(
                validator,
                IFeeManager::setValidatorTokenCall {
                    token: token.address(),
                },
                beneficiary,
            )?;

            // Set user token
            fee_manager.set_user_token(
                user,
                IFeeManager::setUserTokenCall {
                    token: token.address(),
                },
            )?;

            // Call collect_fee_post_tx directly
            let result = fee_manager.collect_fee_post_tx(
                user,
                actual_used,
                refund_amount,
                token.address(),
                validator,
            );
            assert!(result.is_ok());

            // Verify fees were tracked
            let tracked_amount = fee_manager.collected_fees[validator][token.address()].read()?;
            assert_eq!(tracked_amount, actual_used);

            // Verify user got the refund
            let balance = token.balance_of(IMIP20::balanceOfCall { account: user })?;
            assert_eq!(balance, refund_amount);

            Ok(())
        })
    }

    #[test]
    fn test_rejects_non_usd() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let user = Address::random();
        let validator = Address::random();
        let beneficiary = Address::random();
        StorageCtx::enter(&mut storage, || {
            // Create a non-USD token
            let non_usd_token = MIP20Setup::create("NonUSD", "EUR", admin)
                .currency("EUR")
                .apply()?;

            let mut fee_manager = MipFeeManager::new();

            // Try to set non-USD as user token - should fail
            let call = IFeeManager::setUserTokenCall {
                token: non_usd_token.address(),
            };
            let result = fee_manager.set_user_token(user, call);
            assert!(matches!(
                result,
                Err(MagnusPrecompileError::MIP20(MIP20Error::InvalidCurrency(_)))
            ));

            // Try to set non-USD as validator token - should also fail
            let call = IFeeManager::setValidatorTokenCall {
                token: non_usd_token.address(),
            };
            let result = fee_manager.set_validator_token(validator, call, beneficiary);
            assert!(matches!(
                result,
                Err(MagnusPrecompileError::MIP20(MIP20Error::InvalidCurrency(_)))
            ));

            Ok(())
        })
    }

    /// Test collect_fee_pre_tx with different tokens
    /// Verifies that liquidity is checked (not reserved) and no swap happens yet
    #[test]
    fn test_collect_fee_pre_tx_different_tokens() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let user = Address::random();
        let validator = Address::random();

        StorageCtx::enter(&mut storage, || {
            // Create two different tokens
            let user_token = MIP20Setup::create("UserToken", "UTK", admin)
                .with_issuer(admin)
                .with_mint(user, U256::from(10000))
                .with_approval(user, TIP_FEE_MANAGER_ADDRESS, U256::MAX)
                .apply()?;

            let validator_token = MIP20Setup::create("ValidatorToken", "VTK", admin)
                .with_issuer(admin)
                .with_mint(TIP_FEE_MANAGER_ADDRESS, U256::from(10000))
                .apply()?;

            let mut fee_manager = MipFeeManager::new();

            // Setup pool with liquidity
            let pool_id = fee_manager.pool_id(user_token.address(), validator_token.address());
            fee_manager.pools[pool_id].write(crate::mip_fee_manager::amm::Pool {
                reserve_user_token: 10000,
                reserve_validator_token: 10000,
            })?;

            // Set validator's preferred token
            fee_manager.set_validator_token(
                validator,
                IFeeManager::setValidatorTokenCall {
                    token: validator_token.address(),
                },
                Address::random(),
            )?;

            let max_amount = U256::from(1000);

            // Call collect_fee_pre_tx
            fee_manager.collect_fee_pre_tx(
                user,
                user_token.address(),
                max_amount,
                validator,
                false,
            )?;

            // With different tokens:
            // - Liquidity is checked (not reserved)
            // - No swap happens yet (swap happens in collect_fee_post_tx)
            // - collected_fees should be zero
            let collected =
                fee_manager.collected_fees[validator][validator_token.address()].read()?;
            assert_eq!(
                collected,
                U256::ZERO,
                "Different tokens: no fees accumulated in pre_tx (swap happens in post_tx)"
            );

            // Pool reserves should NOT be updated yet
            let pool = fee_manager.pools[pool_id].read()?;
            assert_eq!(
                pool.reserve_user_token, 10000,
                "Reserves unchanged in pre_tx"
            );
            assert_eq!(
                pool.reserve_validator_token, 10000,
                "Reserves unchanged in pre_tx"
            );

            Ok(())
        })
    }

    #[test]
    fn test_collect_fee_post_tx_immediate_swap() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let user = Address::random();
        let validator = Address::random();

        StorageCtx::enter(&mut storage, || {
            let user_token = MIP20Setup::create("UserToken", "UTK", admin)
                .with_issuer(admin)
                .with_mint(user, U256::from(10000))
                .with_mint(TIP_FEE_MANAGER_ADDRESS, U256::from(10000))
                .with_approval(user, TIP_FEE_MANAGER_ADDRESS, U256::MAX)
                .apply()?;

            let validator_token = MIP20Setup::create("ValidatorToken", "VTK", admin)
                .with_issuer(admin)
                .with_mint(TIP_FEE_MANAGER_ADDRESS, U256::from(10000))
                .apply()?;

            let mut fee_manager = MipFeeManager::new();

            let pool_id = fee_manager.pool_id(user_token.address(), validator_token.address());
            fee_manager.pools[pool_id].write(crate::mip_fee_manager::amm::Pool {
                reserve_user_token: 10000,
                reserve_validator_token: 10000,
            })?;

            fee_manager.set_validator_token(
                validator,
                IFeeManager::setValidatorTokenCall {
                    token: validator_token.address(),
                },
                Address::random(),
            )?;

            let max_amount = U256::from(1000);
            let actual_spending = U256::from(800);
            let refund_amount = U256::from(200);

            // First call collect_fee_pre_tx (checks liquidity)
            fee_manager.collect_fee_pre_tx(
                user,
                user_token.address(),
                max_amount,
                validator,
                false,
            )?;

            // Then call collect_fee_post_tx (executes swap immediately)
            fee_manager.collect_fee_post_tx(
                user,
                actual_spending,
                refund_amount,
                user_token.address(),
                validator,
            )?;

            // Expected output: 800 * 9970 / 10000 = 797
            let expected_fee_amount = (actual_spending * U256::from(9970)) / U256::from(10000);
            let collected =
                fee_manager.collected_fees[validator][validator_token.address()].read()?;
            assert_eq!(collected, expected_fee_amount);

            // Pool reserves should be updated
            let pool = fee_manager.pools[pool_id].read()?;
            assert_eq!(pool.reserve_user_token, 10000 + 800);
            assert_eq!(pool.reserve_validator_token, 10000 - 797);

            // User balance: started with 10000, paid 1000 in pre_tx, got 200 refund = 9200
            let mip20_token = MIP20Token::from_address(user_token.address())?;
            let user_balance = mip20_token.balance_of(IMIP20::balanceOfCall { account: user })?;
            assert_eq!(user_balance, U256::from(10000) - max_amount + refund_amount);

            Ok(())
        })
    }

    /// Test collect_fee_pre_tx fails with insufficient liquidity
    #[test]
    fn test_collect_fee_pre_tx_insufficient_liquidity() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let user = Address::random();
        let validator = Address::random();

        StorageCtx::enter(&mut storage, || {
            let user_token = MIP20Setup::create("UserToken", "UTK", admin)
                .with_issuer(admin)
                .with_mint(user, U256::from(10000))
                .with_approval(user, TIP_FEE_MANAGER_ADDRESS, U256::MAX)
                .apply()?;

            let validator_token = MIP20Setup::create("ValidatorToken", "VTK", admin)
                .with_issuer(admin)
                .with_mint(TIP_FEE_MANAGER_ADDRESS, U256::from(100))
                .apply()?;

            let mut fee_manager = MipFeeManager::new();

            let pool_id = fee_manager.pool_id(user_token.address(), validator_token.address());
            // Pool with very little validator token liquidity
            fee_manager.pools[pool_id].write(crate::mip_fee_manager::amm::Pool {
                reserve_user_token: 10000,
                reserve_validator_token: 100,
            })?;

            fee_manager.set_validator_token(
                validator,
                IFeeManager::setValidatorTokenCall {
                    token: validator_token.address(),
                },
                Address::random(),
            )?;

            // Try to collect fee that would require more liquidity than available
            // 1000 * 0.997 = 997 output needed, but only 100 available
            let max_amount = U256::from(1000);

            let result = fee_manager.collect_fee_pre_tx(
                user,
                user_token.address(),
                max_amount,
                validator,
                false,
            );

            assert!(result.is_err(), "Should fail with insufficient liquidity");

            Ok(())
        })
    }

    /// Test that `skip_liquidity_check = true` bypasses the insufficient-liquidity error
    /// when `user_token != validator_token`.
    #[test]
    fn test_collect_fee_pre_tx_skip_liquidity_check() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let user = Address::random();
        let validator = Address::random();

        StorageCtx::enter(&mut storage, || {
            let user_token = MIP20Setup::create("UserToken", "UTK", admin)
                .with_issuer(admin)
                .with_mint(user, U256::from(10000))
                .with_approval(user, TIP_FEE_MANAGER_ADDRESS, U256::MAX)
                .apply()?;

            let validator_token = MIP20Setup::create("ValidatorToken", "VTK", admin)
                .with_issuer(admin)
                .apply()?;

            let mut fee_manager = MipFeeManager::new();
            fee_manager.set_validator_token(
                validator,
                IFeeManager::setValidatorTokenCall {
                    token: validator_token.address(),
                },
                Address::random(),
            )?;

            // Skip liquidity check = false should fail
            let result = fee_manager.collect_fee_pre_tx(
                user,
                user_token.address(),
                U256::from(1000),
                validator,
                false,
            );
            assert!(
                result.is_err(),
                "Should fail without liquidity, got: {result:?}"
            );

            // Skip liquidity check = true should pass
            let result = fee_manager.collect_fee_pre_tx(
                user,
                user_token.address(),
                U256::from(1000),
                validator,
                true,
            );
            assert!(result.is_ok());
            assert_eq!(result?, user_token.address());

            Ok(())
        })
    }

    /// Test distribute_fees with zero balance is a no-op
    #[test]
    fn test_distribute_fees_zero_balance() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let validator = Address::random();

        StorageCtx::enter(&mut storage, || {
            let token = MIP20Setup::create("TestToken", "TEST", admin)
                .with_issuer(admin)
                .apply()?;

            let mut fee_manager = MipFeeManager::new();

            fee_manager.set_validator_token(
                validator,
                IFeeManager::setValidatorTokenCall {
                    token: token.address(),
                },
                Address::random(),
            )?;

            // collected_fees is zero by default
            let collected = fee_manager.collected_fees[validator][token.address()].read()?;
            assert_eq!(collected, U256::ZERO);

            // distribute_fees should be a no-op
            let result = fee_manager.distribute_fees(validator, token.address());
            assert!(result.is_ok(), "Should succeed even with zero balance");

            // Validator balance should still be zero
            let mip20_token = MIP20Token::from_address(token.address())?;
            let balance = mip20_token.balance_of(IMIP20::balanceOfCall { account: validator })?;
            assert_eq!(balance, U256::ZERO);

            Ok(())
        })
    }

    /// Test distribute_fees transfers accumulated fees to validator
    #[test]
    fn test_distribute_fees() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let validator = Address::random();

        StorageCtx::enter(&mut storage, || {
            // Initialize token and give fee manager some tokens
            let token = MIP20Setup::create("TestToken", "TEST", admin)
                .with_issuer(admin)
                .with_mint(TIP_FEE_MANAGER_ADDRESS, U256::from(1000))
                .apply()?;

            let mut fee_manager = MipFeeManager::new();

            // Set validator's preferred token
            fee_manager.set_validator_token(
                validator,
                IFeeManager::setValidatorTokenCall {
                    token: token.address(),
                },
                Address::random(), // beneficiary != validator
            )?;

            // Simulate accumulated fees
            let fee_amount = U256::from(500);
            fee_manager.collected_fees[validator][token.address()].write(fee_amount)?;

            // Check validator balance before
            let mip20_token = MIP20Token::from_address(token.address())?;
            let balance_before =
                mip20_token.balance_of(IMIP20::balanceOfCall { account: validator })?;
            assert_eq!(balance_before, U256::ZERO);

            // Distribute fees
            let mut fee_manager = MipFeeManager::new();
            fee_manager.distribute_fees(validator, token.address())?;

            // Verify validator received the fees
            let mip20_token = MIP20Token::from_address(token.address())?;
            let balance_after =
                mip20_token.balance_of(IMIP20::balanceOfCall { account: validator })?;
            assert_eq!(balance_after, fee_amount);

            // Verify collected fees cleared
            let fee_manager = MipFeeManager::new();
            let remaining = fee_manager.collected_fees[validator][token.address()].read()?;
            assert_eq!(remaining, U256::ZERO);

            Ok(())
        })
    }

    #[test]
    fn test_initialize_sets_storage_state() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut fee_manager = MipFeeManager::new();

            // Before init, should not be initialized
            assert!(!fee_manager.is_initialized()?);

            // Initialize
            fee_manager.initialize()?;

            // After init, should be initialized
            assert!(fee_manager.is_initialized()?);

            // New handle should still see initialized state
            let fee_manager2 = MipFeeManager::new();
            assert!(fee_manager2.is_initialized()?);

            Ok(())
        })
    }
}

// ─── G1: Currency registry tests (multi-currency-fees-design.md §4) ──────────
#[cfg(test)]
mod currency_registry_tests {
    use super::*;
    use crate::{
        error::MagnusPrecompileError,
        storage::{ContractStorage, StorageCtx, hashmap::HashMapStorageProvider},
        test_util::MIP20Setup,
    };

    /// Bootstrap helper: a fresh FeeManager with `governance_admin` set to `admin` via the
    /// zero-address bootstrap path. Mirrors what the genesis-init transaction does at T4
    /// activation.
    fn fee_manager_with_admin(admin: Address) -> Result<MipFeeManager> {
        let mut fee_manager = MipFeeManager::new();
        fee_manager.initialize()?;
        // Bootstrap: admin == zero, so any caller can set the initial admin.
        fee_manager.set_governance_admin(Address::ZERO, admin)?;
        Ok(fee_manager)
    }

    #[test]
    fn governance_admin_zero_at_genesis() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let fee_manager = MipFeeManager::new();
            assert_eq!(fee_manager.governance_admin()?, Address::ZERO);
            Ok(())
        })
    }

    #[test]
    fn set_governance_admin_bootstrap_from_zero_succeeds() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let any_caller = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fee_manager = MipFeeManager::new();
            // Zero-address bootstrap: anyone can set the initial admin.
            fee_manager.set_governance_admin(any_caller, admin)?;
            assert_eq!(fee_manager.governance_admin()?, admin);
            Ok(())
        })
    }

    #[test]
    fn set_governance_admin_rotates_only_via_current_admin() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::repeat_byte(0xAA);
        let new_admin = Address::repeat_byte(0xBB);
        let attacker = Address::repeat_byte(0xCC);
        StorageCtx::enter(&mut storage, || {
            let mut fee_manager = fee_manager_with_admin(admin)?;

            // Non-admin cannot rotate.
            let err = fee_manager
                .set_governance_admin(attacker, new_admin)
                .unwrap_err();
            assert_eq!(
                err,
                MagnusPrecompileError::FeeManagerError(FeeManagerError::only_governance_admin(
                    attacker
                ))
            );

            // Current admin can rotate.
            fee_manager.set_governance_admin(admin, new_admin)?;
            assert_eq!(fee_manager.governance_admin()?, new_admin);
            Ok(())
        })
    }

    #[test]
    fn set_governance_admin_rejects_zero_address() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fee_manager = fee_manager_with_admin(admin)?;
            let err = fee_manager
                .set_governance_admin(admin, Address::ZERO)
                .unwrap_err();
            assert_eq!(
                err,
                MagnusPrecompileError::FeeManagerError(
                    FeeManagerError::zero_address_governance_admin()
                )
            );
            Ok(())
        })
    }

    #[test]
    fn add_currency_happy_path_emits_event_and_persists() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fee_manager = fee_manager_with_admin(admin)?;
            fee_manager.clear_emitted_events();

            fee_manager.add_currency(admin, "USD", 100)?;

            // Persisted: registered, not yet enabled.
            let config = fee_manager.get_currency_config("USD")?;
            assert!(!config.enabled);
            assert_eq!(config.added_at_block, 100);
            assert_eq!(config.enabled_at_block, 0);
            assert!(!fee_manager.is_currency_enabled("USD")?);

            // Event emitted.
            fee_manager.assert_emitted_events(vec![FeeManagerEvent::CurrencyAdded(
                IFeeManager::CurrencyAdded {
                    code: "USD".into(),
                    atBlock: 100,
                },
            )]);
            Ok(())
        })
    }

    #[test]
    fn add_currency_rejects_non_admin() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let attacker = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fee_manager = fee_manager_with_admin(admin)?;
            let err = fee_manager.add_currency(attacker, "USD", 0).unwrap_err();
            assert_eq!(
                err,
                MagnusPrecompileError::FeeManagerError(FeeManagerError::only_governance_admin(
                    attacker
                ))
            );
            Ok(())
        })
    }

    #[test]
    fn add_currency_rejects_invalid_code() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fee_manager = fee_manager_with_admin(admin)?;
            for bad in &["", "us", "usd", "USDT", "US1"] {
                let err = fee_manager.add_currency(admin, bad, 0).unwrap_err();
                assert_eq!(
                    err,
                    MagnusPrecompileError::FeeManagerError(FeeManagerError::invalid_currency_code(
                        (*bad).into()
                    )),
                    "expected InvalidCurrencyCode for {:?}",
                    bad
                );
            }
            Ok(())
        })
    }

    #[test]
    fn add_currency_rejects_double_add() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fee_manager = fee_manager_with_admin(admin)?;
            fee_manager.add_currency(admin, "USD", 0)?;
            let err = fee_manager.add_currency(admin, "USD", 1).unwrap_err();
            assert_eq!(
                err,
                MagnusPrecompileError::FeeManagerError(FeeManagerError::currency_already_added(
                    "USD".into()
                ))
            );
            Ok(())
        })
    }

    #[test]
    fn enable_currency_happy_path_flips_flag_and_records_block() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fee_manager = fee_manager_with_admin(admin)?;
            fee_manager.add_currency(admin, "USD", 50)?;
            fee_manager.clear_emitted_events();

            fee_manager.enable_currency(admin, "USD", 200)?;

            let config = fee_manager.get_currency_config("USD")?;
            assert!(config.enabled);
            assert_eq!(config.added_at_block, 50);
            assert_eq!(config.enabled_at_block, 200);
            assert!(fee_manager.is_currency_enabled("USD")?);

            fee_manager.assert_emitted_events(vec![FeeManagerEvent::CurrencyEnabled(
                IFeeManager::CurrencyEnabled {
                    code: "USD".into(),
                    atBlock: 200,
                },
            )]);
            Ok(())
        })
    }

    #[test]
    fn enable_currency_rejects_unregistered() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fee_manager = fee_manager_with_admin(admin)?;
            let err = fee_manager.enable_currency(admin, "VND", 0).unwrap_err();
            assert_eq!(
                err,
                MagnusPrecompileError::FeeManagerError(FeeManagerError::currency_not_registered(
                    "VND".into()
                ))
            );
            Ok(())
        })
    }

    #[test]
    fn enable_currency_rejects_already_enabled() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fee_manager = fee_manager_with_admin(admin)?;
            fee_manager.add_currency(admin, "USD", 0)?;
            fee_manager.enable_currency(admin, "USD", 1)?;
            let err = fee_manager.enable_currency(admin, "USD", 2).unwrap_err();
            assert_eq!(
                err,
                MagnusPrecompileError::FeeManagerError(FeeManagerError::currency_already_enabled(
                    "USD".into()
                ))
            );
            Ok(())
        })
    }

    #[test]
    fn enable_currency_rejects_non_admin() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let attacker = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fee_manager = fee_manager_with_admin(admin)?;
            fee_manager.add_currency(admin, "USD", 0)?;
            let err = fee_manager.enable_currency(attacker, "USD", 1).unwrap_err();
            assert_eq!(
                err,
                MagnusPrecompileError::FeeManagerError(FeeManagerError::only_governance_admin(
                    attacker
                ))
            );
            Ok(())
        })
    }

    #[test]
    fn unregistered_currency_returns_default_config() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let fee_manager = MipFeeManager::new();
            let config = fee_manager.get_currency_config("USD")?;
            assert!(!config.enabled);
            assert_eq!(config.added_at_block, 0);
            assert_eq!(config.enabled_at_block, 0);
            assert!(!fee_manager.is_currency_enabled("USD")?);
            Ok(())
        })
    }

    // ─── G2a: validator accept-set API tests ──────────────────────────────────

    /// Bootstrap fee manager with admin + USD enabled so add_accepted_token can validate
    /// against the currency registry.
    fn fee_manager_with_admin_and_usd(admin: Address) -> Result<MipFeeManager> {
        let mut fm = fee_manager_with_admin(admin)?;
        fm.add_currency(admin, "USD", 0)?;
        fm.enable_currency(admin, "USD", 0)?;
        Ok(fm)
    }

    #[test]
    fn add_accepted_token_happy_path() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let validator = Address::random();
        let beneficiary = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_with_admin_and_usd(admin)?;
            let token = MIP20Setup::create("USDC", "USDC", admin)
                .currency("USD")
                .apply()?;
            fm.clear_emitted_events();

            fm.add_accepted_token(validator, token.address(), beneficiary)?;

            assert!(fm.accepts_token(validator, token.address())?);
            let list = fm.get_accepted_tokens(validator)?;
            assert_eq!(list, vec![token.address()]);

            fm.assert_emitted_events(vec![FeeManagerEvent::AcceptedTokenAdded(
                IFeeManager::AcceptedTokenAdded {
                    validator,
                    token: token.address(),
                },
            )]);
            Ok(())
        })
    }

    #[test]
    fn add_accepted_token_rejects_non_tip20() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let validator = Address::random();
        let beneficiary = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_with_admin_and_usd(admin)?;
            let bogus = Address::random();

            let err = fm
                .add_accepted_token(validator, bogus, beneficiary)
                .unwrap_err();
            assert_eq!(
                err,
                MagnusPrecompileError::FeeManagerError(FeeManagerError::invalid_token())
            );
            Ok(())
        })
    }

    #[test]
    fn add_accepted_token_rejects_unregistered_currency_token() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let validator = Address::random();
        let beneficiary = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_with_admin(admin)?;
            // USD never registered.
            let token = MIP20Setup::create("USDC", "USDC", admin)
                .currency("USD")
                .apply()?;

            let err = fm
                .add_accepted_token(validator, token.address(), beneficiary)
                .unwrap_err();
            assert!(matches!(
                err,
                MagnusPrecompileError::FeeManagerError(FeeManagerError::CurrencyNotRegistered(_))
            ));
            Ok(())
        })
    }

    #[test]
    fn add_accepted_token_rejects_same_block_as_beneficiary() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let validator = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_with_admin_and_usd(admin)?;
            let token = MIP20Setup::create("USDC", "USDC", admin)
                .currency("USD")
                .apply()?;

            let err = fm
                .add_accepted_token(validator, token.address(), validator)
                .unwrap_err();
            assert_eq!(
                err,
                MagnusPrecompileError::FeeManagerError(
                    FeeManagerError::cannot_change_within_block()
                )
            );
            Ok(())
        })
    }

    #[test]
    fn add_accepted_token_rejects_double_add() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let validator = Address::random();
        let beneficiary = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_with_admin_and_usd(admin)?;
            let token = MIP20Setup::create("USDC", "USDC", admin)
                .currency("USD")
                .apply()?;

            fm.add_accepted_token(validator, token.address(), beneficiary)?;
            let err = fm
                .add_accepted_token(validator, token.address(), beneficiary)
                .unwrap_err();
            assert_eq!(
                err,
                MagnusPrecompileError::FeeManagerError(FeeManagerError::token_already_accepted(
                    validator,
                    token.address()
                ))
            );
            Ok(())
        })
    }

    #[test]
    fn add_accepted_token_enforces_max_size() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let validator = Address::random();
        let beneficiary = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_with_admin_and_usd(admin)?;

            // Fill the accept-set to MAX_ACCEPT_SET_SIZE. Use distinct salts so the
            // factory derives different deterministic addresses; the underlying name
            // doesn't need to be unique for the storage layer.
            for i in 0..MipFeeManager::MAX_ACCEPT_SET_SIZE {
                let token = MIP20Setup::create("USDC", "USDC", admin)
                    .currency("USD")
                    .with_salt(B256::from(U256::from(i as u64)))
                    .apply()?;
                fm.add_accepted_token(validator, token.address(), beneficiary)?;
            }

            // The 33rd add must revert.
            let extra_token = MIP20Setup::create("USDC", "USDC", admin)
                .currency("USD")
                .with_salt(B256::from(U256::from(
                    MipFeeManager::MAX_ACCEPT_SET_SIZE as u64,
                )))
                .apply()?;
            let err = fm
                .add_accepted_token(validator, extra_token.address(), beneficiary)
                .unwrap_err();
            assert_eq!(
                err,
                MagnusPrecompileError::FeeManagerError(FeeManagerError::max_accept_set_reached(
                    validator
                ))
            );
            Ok(())
        })
    }

    #[test]
    fn remove_accepted_token_happy_path() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let validator = Address::random();
        let beneficiary = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_with_admin_and_usd(admin)?;
            let token = MIP20Setup::create("USDC", "USDC", admin)
                .currency("USD")
                .apply()?;
            fm.add_accepted_token(validator, token.address(), beneficiary)?;
            fm.clear_emitted_events();

            fm.remove_accepted_token(validator, token.address(), beneficiary)?;

            assert!(!fm.accepts_token(validator, token.address())?);
            assert!(fm.get_accepted_tokens(validator)?.is_empty());

            fm.assert_emitted_events(vec![FeeManagerEvent::AcceptedTokenRemoved(
                IFeeManager::AcceptedTokenRemoved {
                    validator,
                    token: token.address(),
                },
            )]);
            Ok(())
        })
    }

    #[test]
    fn remove_accepted_token_rejects_unaccepted() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let validator = Address::random();
        let beneficiary = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_with_admin_and_usd(admin)?;
            let token = MIP20Setup::create("USDC", "USDC", admin)
                .currency("USD")
                .apply()?;

            let err = fm
                .remove_accepted_token(validator, token.address(), beneficiary)
                .unwrap_err();
            assert_eq!(
                err,
                MagnusPrecompileError::FeeManagerError(FeeManagerError::token_not_in_accept_set(
                    validator,
                    token.address()
                ))
            );
            Ok(())
        })
    }

    #[test]
    fn remove_accepted_token_keeps_other_tokens_in_list() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let validator = Address::random();
        let beneficiary = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_with_admin_and_usd(admin)?;
            // Distinct salts ensure distinct deterministic factory addresses.
            let usdc = MIP20Setup::create("USDC", "USDC", admin)
                .currency("USD")
                .with_salt(B256::from(U256::from(1)))
                .apply()?;
            let usdt = MIP20Setup::create("USDT", "USDT", admin)
                .currency("USD")
                .with_salt(B256::from(U256::from(2)))
                .apply()?;
            let pyusd = MIP20Setup::create("PYUSD", "PYUSD", admin)
                .currency("USD")
                .with_salt(B256::from(U256::from(3)))
                .apply()?;

            fm.add_accepted_token(validator, usdc.address(), beneficiary)?;
            fm.add_accepted_token(validator, usdt.address(), beneficiary)?;
            fm.add_accepted_token(validator, pyusd.address(), beneficiary)?;

            // Remove USDT (the middle one). Order is allowed to change (swap-remove).
            fm.remove_accepted_token(validator, usdt.address(), beneficiary)?;

            assert!(fm.accepts_token(validator, usdc.address())?);
            assert!(!fm.accepts_token(validator, usdt.address())?);
            assert!(fm.accepts_token(validator, pyusd.address())?);

            let list = fm.get_accepted_tokens(validator)?;
            assert_eq!(list.len(), 2);
            assert!(list.contains(&usdc.address()));
            assert!(list.contains(&pyusd.address()));
            Ok(())
        })
    }

    #[test]
    fn accepts_token_returns_false_for_unconfigured_validator() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let validator = Address::random();
        let token = Address::random();
        StorageCtx::enter(&mut storage, || {
            let fm = MipFeeManager::new();
            assert!(!fm.accepts_token(validator, token)?);
            assert!(fm.get_accepted_tokens(validator)?.is_empty());
            Ok(())
        })
    }

    #[test]
    fn add_then_remove_then_re_add_works() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let validator = Address::random();
        let beneficiary = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_with_admin_and_usd(admin)?;
            let token = MIP20Setup::create("USDC", "USDC", admin)
                .currency("USD")
                .apply()?;

            fm.add_accepted_token(validator, token.address(), beneficiary)?;
            fm.remove_accepted_token(validator, token.address(), beneficiary)?;
            // Re-add must succeed cleanly (state was fully cleaned up).
            fm.add_accepted_token(validator, token.address(), beneficiary)?;

            assert!(fm.accepts_token(validator, token.address())?);
            assert_eq!(fm.get_accepted_tokens(validator)?, vec![token.address()]);
            Ok(())
        })
    }

    #[test]
    fn multiple_validators_have_independent_accept_sets() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let v1 = Address::random();
        let v2 = Address::random();
        let beneficiary = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_with_admin_and_usd(admin)?;
            let usdc = MIP20Setup::create("USDC", "USDC", admin)
                .currency("USD")
                .with_salt(B256::from(U256::from(11)))
                .apply()?;
            let usdt = MIP20Setup::create("USDT", "USDT", admin)
                .currency("USD")
                .with_salt(B256::from(U256::from(12)))
                .apply()?;

            fm.add_accepted_token(v1, usdc.address(), beneficiary)?;
            fm.add_accepted_token(v1, usdt.address(), beneficiary)?;
            fm.add_accepted_token(v2, usdc.address(), beneficiary)?;

            assert!(fm.accepts_token(v1, usdc.address())?);
            assert!(fm.accepts_token(v1, usdt.address())?);
            assert!(fm.accepts_token(v2, usdc.address())?);
            assert!(!fm.accepts_token(v2, usdt.address())?);

            assert_eq!(fm.get_accepted_tokens(v1)?.len(), 2);
            assert_eq!(fm.get_accepted_tokens(v2)?.len(), 1);
            Ok(())
        })
    }

    #[test]
    fn is_accepted_by_any_validator_g2a_stub_always_false() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let validator = Address::random();
        let beneficiary = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_with_admin_and_usd(admin)?;
            let token = MIP20Setup::create("USDC", "USDC", admin)
                .currency("USD")
                .apply()?;
            fm.add_accepted_token(validator, token.address(), beneficiary)?;

            // G2a stub: always false even though the token IS accepted by `validator`.
            // G2b/G4 may add a real reverse-index. Pin the stub behavior so a future
            // accidental change is caught.
            assert!(!fm.is_accepted_by_any_validator(token.address())?);
            Ok(())
        })
    }

    #[test]
    fn multiple_currencies_can_be_independently_managed() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fee_manager = fee_manager_with_admin(admin)?;
            fee_manager.add_currency(admin, "USD", 10)?;
            fee_manager.add_currency(admin, "VND", 20)?;
            fee_manager.add_currency(admin, "EUR", 30)?;
            fee_manager.enable_currency(admin, "USD", 100)?;
            fee_manager.enable_currency(admin, "VND", 200)?;
            // EUR registered but not enabled.

            assert!(fee_manager.is_currency_enabled("USD")?);
            assert!(fee_manager.is_currency_enabled("VND")?);
            assert!(!fee_manager.is_currency_enabled("EUR")?);
            assert!(!fee_manager.is_currency_enabled("GBP")?); // never added

            let usd = fee_manager.get_currency_config("USD")?;
            assert_eq!(usd.added_at_block, 10);
            assert_eq!(usd.enabled_at_block, 100);
            let vnd = fee_manager.get_currency_config("VND")?;
            assert_eq!(vnd.added_at_block, 20);
            assert_eq!(vnd.enabled_at_block, 200);
            let eur = fee_manager.get_currency_config("EUR")?;
            assert_eq!(eur.added_at_block, 30);
            assert_eq!(eur.enabled_at_block, 0);
            Ok(())
        })
    }
}
