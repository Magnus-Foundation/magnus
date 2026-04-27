//! [Fee manager] precompile for transaction fee collection, distribution, and token swaps.
//!
//! [Fee manager]: <https://docs.magnus.xyz/protocol/fees>

pub mod amm;
pub mod currency_registry;
pub mod dispatch;
pub mod escrow;

use crate::{
    error::{MagnusPrecompileError, Result},
    mip_fee_manager::amm::{Pool, PoolKey, compute_amount_out},
    mip_fee_manager::currency_registry::{CurrencyConfig, currency_key, is_valid_currency_code},
    mip_fee_manager::escrow::ClaimRecord,
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

    /// Address authorized to call governance-gated setters (currency registry).
    /// Intended to be a multisig contract; signature aggregation lives at that layer.
    governance_admin: Address,
    /// Per-currency config. Key = `keccak256(ISO 4217 code)`.
    supported_currencies: Mapping<B256, CurrencyConfig>,
    /// Seconds between `disable_currency` and the currency becoming disabled.
    /// Zero (default) means use `DEFAULT_GRACE_PERIOD_SECS`.
    deprecation_grace_period: u64,
    /// Multisig threshold required for `emergency_disable_currency`. Zero (default)
    /// means use `DEFAULT_EMERGENCY_THRESHOLD`. Forward-compat metadata until
    /// EIP-712 multisig governance lands.
    emergency_disable_threshold: u8,
    /// Reverse index for `prune_currency`: token → list of validators that have
    /// added it to their accept-set. Maintained by add/remove_accepted_token.
    token_validators: Mapping<Address, Vec<Address>>,

    /// Off-boarding escrow recipient when direct fee transfer to a deactivated
    /// validator fails. Zero = unconfigured (must be set by genesis or governance
    /// before sweep).
    foundation_escrow_address: Address,
    /// Escrowed fees by (validator, token) when off-board direct delivery failed.
    escrowed_fees: Mapping<Address, Mapping<Address, U256>>,
    /// Per-validator off-board record (deadline tracking).
    escrow_claims: Mapping<Address, ClaimRecord>,
    /// Seconds the validator has to claim escrowed fees. Zero (default) means
    /// `DEFAULT_ESCROW_CLAIM_WINDOW_SECS`.
    escrow_claim_window: u64,

    /// Validator → token → accepted? Multi-token accept-set replacing the legacy
    /// single-token `validator_tokens` model. Map + list stay in sync.
    validator_accepted_tokens: Mapping<Address, Mapping<Address, bool>>,
    /// Validator → enumeration of accepted tokens. Mirror of the map above.
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

    /// Cap on per-validator accept-set size. Prevents state-bloat abuse.
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
        if self.storage.spec().is_t4() {
            return Err(FeeManagerError::user_token_api_removed().into());
        }

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
    /// T4+: validator must have `fee_token` in its accept-set; direct-credit only,
    /// no AMM swap. Pre-T4: legacy single-token preference + AMM swap fallback.
    ///
    /// # Errors
    /// - `InvalidToken` — `fee_token` does not have a valid MIP-20 prefix
    /// - `FeeTokenNotAccepted` (T4+) — validator's accept-set does not include `fee_token`
    /// - `InsufficientLiquidity` (pre-T4) — AMM pool lacks liquidity for the fee swap
    /// - `UnderOverflow` — collected-fee accumulator overflows
    pub fn collect_fee_post_tx(
        &mut self,
        fee_payer: Address,
        actual_spending: U256,
        refund_amount: U256,
        fee_token: Address,
        beneficiary: Address,
    ) -> Result<()> {
        let mut mip20_token = MIP20Token::from_address(fee_token)?;
        mip20_token.transfer_fee_post_tx(fee_payer, refund_amount, actual_spending)?;

        if self.storage.spec().is_t4() {
            return self.settle_fee_t4(beneficiary, fee_token, actual_spending);
        }

        // Legacy pre-T4 path: AMM swap when user/validator tokens differ.
        let validator_token = self.get_validator_token(beneficiary)?;

        if fee_token != validator_token && !actual_spending.is_zero() {
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

    /// T4+ fee settlement: direct-credit if validator accepts `fee_token`, else revert.
    /// No AMM swap, no currency conversion.
    fn settle_fee_t4(
        &mut self,
        beneficiary: Address,
        fee_token: Address,
        actual_spending: U256,
    ) -> Result<()> {
        if !self.accepts_token(beneficiary, fee_token)? {
            return Err(FeeManagerError::fee_token_not_accepted(beneficiary, fee_token).into());
        }
        self.increment_collected_fees(beneficiary, fee_token, actual_spending)?;
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

    /// Reads the stored fee token preference for a user. Returns zero on T4+ (API removed).
    pub fn user_tokens(&self, call: IFeeManager::userTokensCall) -> Result<Address> {
        if self.storage.spec().is_t4() {
            return Ok(Address::ZERO);
        }
        self.user_tokens[call.user].read()
    }

    // Currency registry: governance-gated setters use `sender == governance_admin`.

    /// Address authorized to call governance setters. Zero = unconfigured.
    pub fn governance_admin(&self) -> Result<Address> {
        self.governance_admin.read()
    }

    /// Returns default-zero config for unregistered codes.
    pub fn get_currency_config(&self, code: &str) -> Result<CurrencyConfig> {
        self.supported_currencies[currency_key(code)].read()
    }

    pub fn is_currency_enabled(&self, code: &str) -> Result<bool> {
        Ok(self.get_currency_config(code)?.enabled)
    }

    /// Registers a currency in disabled state. Must be `enable`d separately.
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

    /// Marks a registered currency as gas-eligible.
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
        // Re-enabling clears any prior deprecation flags.
        config.deprecating = false;
        config.deprecation_activates_at = 0;
        self.supported_currencies[key].write(config)?;

        self.emit_event(FeeManagerEvent::CurrencyEnabled(IFeeManager::CurrencyEnabled {
            code: code.into(),
            atBlock: current_block,
        }))?;

        Ok(())
    }

    /// Genesis default for `deprecation_grace_period`: 30 days in seconds.
    pub const DEFAULT_GRACE_PERIOD_SECS: u64 = 30 * 24 * 60 * 60;
    /// Sanity-bound minimum grace period: 1 hour.
    pub const MIN_GRACE_PERIOD_SECS: u64 = 60 * 60;
    /// Sanity-bound maximum grace period: 365 days.
    pub const MAX_GRACE_PERIOD_SECS: u64 = 365 * 24 * 60 * 60;

    /// Effective grace period; falls back to the default when storage is zero.
    pub fn deprecation_grace_period(&self) -> Result<u64> {
        let raw = self.deprecation_grace_period.read()?;
        Ok(if raw == 0 {
            Self::DEFAULT_GRACE_PERIOD_SECS
        } else {
            raw
        })
    }

    /// Sets the grace period. Caller must be `governance_admin`. Bounded to
    /// `[MIN_GRACE_PERIOD_SECS, MAX_GRACE_PERIOD_SECS]`.
    pub fn set_deprecation_grace_period(
        &mut self,
        sender: Address,
        new_grace: u64,
    ) -> Result<()> {
        self.assert_governance(sender)?;
        if !(Self::MIN_GRACE_PERIOD_SECS..=Self::MAX_GRACE_PERIOD_SECS).contains(&new_grace) {
            return Err(FeeManagerError::grace_period_out_of_range(new_grace).into());
        }
        let old = self.deprecation_grace_period.read()?;
        self.deprecation_grace_period.write(new_grace)?;
        self.emit_event(FeeManagerEvent::DeprecationGracePeriodChanged(
            IFeeManager::DeprecationGracePeriodChanged {
                oldGracePeriod: old,
                newGracePeriod: new_grace,
            },
        ))?;
        Ok(())
    }

    /// Starts the deprecation grace period for `code`. After the grace window
    /// the currency becomes effectively disabled; existing fees can still settle
    /// in the meantime, but new factory deploys and accept-set adds are blocked.
    pub fn disable_currency(
        &mut self,
        sender: Address,
        code: &str,
        now_ts: u64,
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
        if !config.enabled {
            return Err(FeeManagerError::currency_disabled(code.into()).into());
        }
        if config.deprecating {
            return Err(FeeManagerError::currency_already_deprecating(code.into()).into());
        }

        let grace = self.deprecation_grace_period()?;
        let ends_at = now_ts.saturating_add(grace);

        config.deprecating = true;
        config.deprecation_activates_at = ends_at;
        self.supported_currencies[key].write(config)?;

        self.emit_event(FeeManagerEvent::CurrencyDisabling(
            IFeeManager::CurrencyDisabling {
                code: code.into(),
                graceEndsAt: ends_at,
                by: sender,
            },
        ))?;
        Ok(())
    }

    /// Lazy effective-enabled check at `now_ts` (timestamp in seconds).
    pub fn is_currency_effectively_enabled(&self, code: &str, now_ts: u64) -> Result<bool> {
        Ok(self.get_currency_config(code)?.effectively_enabled(now_ts))
    }

    /// True iff `code` is currently in the deprecation grace window.
    pub fn is_currency_in_grace(&self, code: &str, now_ts: u64) -> Result<bool> {
        Ok(self.get_currency_config(code)?.in_grace_period(now_ts))
    }

    /// Rotates governance authority. Bootstrap: when current admin is zero, any caller may set.
    pub fn set_governance_admin(&mut self, sender: Address, new_admin: Address) -> Result<()> {
        if new_admin.is_zero() {
            return Err(FeeManagerError::zero_address_governance_admin().into());
        }
        let old_admin = self.governance_admin.read()?;
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

    fn assert_governance(&self, sender: Address) -> Result<()> {
        let admin = self.governance_admin.read()?;
        if sender != admin {
            return Err(FeeManagerError::only_governance_admin(sender).into());
        }
        Ok(())
    }

    // Validator multi-token accept-set API.

    pub fn accepts_token(&self, validator: Address, token: Address) -> Result<bool> {
        self.validator_accepted_tokens[validator][token].read()
    }

    pub fn get_accepted_tokens(&self, validator: Address) -> Result<Vec<Address>> {
        self.validator_token_list[validator].read()
    }

    /// True iff at least one validator has `token` in their accept-set. Backed by
    /// the `token_validators` reverse index maintained by add/remove_accepted_token.
    pub fn is_accepted_by_any_validator(&self, token: Address) -> Result<bool> {
        Ok(!self.token_validators[token].read()?.is_empty())
    }

    /// Adds `token` to the caller's accept-set.
    pub fn add_accepted_token(
        &mut self,
        sender: Address,
        token: Address,
        beneficiary: Address,
    ) -> Result<()> {
        if !MIP20Factory::new().is_tip20(token)? {
            return Err(FeeManagerError::invalid_token().into());
        }
        crate::mip20::validate_supported_currency(token)?;

        // Block adds during a deprecation grace window. Validators may keep their
        // existing accept-set entries — only new entries are blocked.
        let currency = MIP20Token::from_address(token)?.currency()?;
        let now_ts = self.storage.timestamp().saturating_to::<u64>();
        let cfg = self.get_currency_config(&currency)?;
        if cfg.in_grace_period(now_ts) {
            return Err(FeeManagerError::currency_deprecating(
                currency,
                cfg.deprecation_activates_at,
            )
            .into());
        }

        if sender == beneficiary {
            return Err(FeeManagerError::cannot_change_within_block().into());
        }
        if self.validator_accepted_tokens[sender][token].read()? {
            return Err(FeeManagerError::token_already_accepted(sender, token).into());
        }

        let mut list = self.validator_token_list[sender].read()?;
        if list.len() >= Self::MAX_ACCEPT_SET_SIZE {
            return Err(FeeManagerError::max_accept_set_reached(sender).into());
        }
        list.push(token);
        self.validator_token_list[sender].write(list)?;
        self.validator_accepted_tokens[sender][token].write(true)?;

        let mut validators = self.token_validators[token].read()?;
        validators.push(sender);
        self.token_validators[token].write(validators)?;

        self.emit_event(FeeManagerEvent::AcceptedTokenAdded(
            IFeeManager::AcceptedTokenAdded {
                validator: sender,
                token,
            },
        ))?;
        Ok(())
    }

    /// Removes `token` from the caller's accept-set.
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

        // Map and list must stay in sync; mismatch is a real bug, not silent cleanup.
        let mut list = self.validator_token_list[sender].read()?;
        let pos = list.iter().position(|t| *t == token).ok_or_else(|| {
            MagnusPrecompileError::Fatal(
                "validator_accepted_tokens flag set but token missing from list".into(),
            )
        })?;
        list.swap_remove(pos);
        self.validator_token_list[sender].write(list)?;
        self.validator_accepted_tokens[sender][token].write(false)?;

        let mut validators = self.token_validators[token].read()?;
        if let Some(vpos) = validators.iter().position(|v| *v == sender) {
            validators.swap_remove(vpos);
            self.token_validators[token].write(validators)?;
        }

        self.emit_event(FeeManagerEvent::AcceptedTokenRemoved(
            IFeeManager::AcceptedTokenRemoved {
                validator: sender,
                token,
            },
        ))?;
        Ok(())
    }

    /// Genesis default for `emergency_disable_threshold`: 7 (of a 9-signer multisig).
    pub const DEFAULT_EMERGENCY_THRESHOLD: u8 = 7;
    /// Sanity-bound minimum emergency threshold: 6 (must exceed standard 5-of-9).
    pub const MIN_EMERGENCY_THRESHOLD: u8 = 6;
    /// Sanity-bound maximum emergency threshold: 9.
    pub const MAX_EMERGENCY_THRESHOLD: u8 = 9;
    /// Cap on per-call prune iterations (gas safety).
    pub const MAX_PRUNE_ITERATIONS: u64 = 256;

    /// Effective emergency threshold; falls back to the default when storage is zero.
    pub fn emergency_disable_threshold(&self) -> Result<u8> {
        let raw = self.emergency_disable_threshold.read()?;
        Ok(if raw == 0 {
            Self::DEFAULT_EMERGENCY_THRESHOLD
        } else {
            raw
        })
    }

    /// Updates the emergency threshold. Sanity-bound to `[MIN, MAX]`. Single-admin
    /// gate today; intended to require the *current* emergency threshold once
    /// EIP-712 multisig governance lands (spec §11.1).
    pub fn set_emergency_disable_threshold(
        &mut self,
        sender: Address,
        new_threshold: u8,
    ) -> Result<()> {
        self.assert_governance(sender)?;
        if !(Self::MIN_EMERGENCY_THRESHOLD..=Self::MAX_EMERGENCY_THRESHOLD).contains(&new_threshold)
        {
            return Err(FeeManagerError::emergency_threshold_out_of_range(new_threshold).into());
        }
        let old = self.emergency_disable_threshold()?;
        self.emergency_disable_threshold.write(new_threshold)?;
        self.emit_event(FeeManagerEvent::EmergencyDisableThresholdChanged(
            IFeeManager::EmergencyDisableThresholdChanged {
                oldThreshold: old,
                newThreshold: new_threshold,
            },
        ))?;
        Ok(())
    }

    /// Immediately flips a currency to disabled (no grace period). Intended for
    /// security/regulatory crises. Single-admin gate today.
    pub fn emergency_disable_currency(&mut self, sender: Address, code: &str) -> Result<()> {
        self.assert_governance(sender)?;
        if !is_valid_currency_code(code) {
            return Err(FeeManagerError::invalid_currency_code(code.into()).into());
        }
        let key = currency_key(code);
        let mut config = self.supported_currencies[key].read()?;
        if !config.registered {
            return Err(FeeManagerError::currency_not_registered(code.into()).into());
        }
        if !config.enabled {
            return Err(FeeManagerError::currency_disabled(code.into()).into());
        }

        config.enabled = false;
        config.deprecating = false;
        config.deprecation_activates_at = 0;
        self.supported_currencies[key].write(config)?;

        self.emit_event(FeeManagerEvent::CurrencyDisabledEmergency(
            IFeeManager::CurrencyDisabledEmergency {
                code: code.into(),
                by: sender,
            },
        ))?;
        Ok(())
    }

    /// Marks `code` as freshly pruned at `current_block`. Permissionless. The
    /// currency must be disabled (emergency-flipped or expired grace). Per-token
    /// state cleanup happens via `prune_token`; this call only stamps the
    /// timestamp and emits `CurrencyPruned` for off-chain coordination.
    pub fn prune_currency(
        &mut self,
        code: &str,
        _max_iterations: u64,
        current_block: u64,
    ) -> Result<()> {
        if !is_valid_currency_code(code) {
            return Err(FeeManagerError::invalid_currency_code(code.into()).into());
        }
        let key = currency_key(code);
        let mut config = self.supported_currencies[key].read()?;
        if !config.registered {
            return Err(FeeManagerError::currency_not_registered(code.into()).into());
        }
        let now_ts = self.storage.timestamp().saturating_to::<u64>();
        if config.effectively_enabled(now_ts) {
            return Err(FeeManagerError::currency_not_disabled(code.into()).into());
        }

        config.last_pruned_at_block = current_block;
        self.supported_currencies[key].write(config)?;

        self.emit_event(FeeManagerEvent::CurrencyPruned(IFeeManager::CurrencyPruned {
            code: code.into(),
            tokensRemoved: U256::ZERO,
            atBlock: current_block,
        }))?;
        Ok(())
    }

    /// Per-token prune: removes `token` from every validator's accept-set.
    /// Permissionless. Currency of `token` must be disabled. Returns the
    /// count of validator entries removed (capped by `max_iterations`).
    pub fn prune_token(
        &mut self,
        token: Address,
        max_iterations: u64,
        current_block: u64,
    ) -> Result<u64> {
        if !MIP20Factory::new().is_tip20(token)? {
            return Err(FeeManagerError::invalid_token().into());
        }
        let currency = MIP20Token::from_address(token)?.currency()?;
        let key = currency_key(&currency);
        let mut config = self.supported_currencies[key].read()?;
        if !config.registered {
            return Err(FeeManagerError::currency_not_registered(currency).into());
        }
        let now_ts = self.storage.timestamp().saturating_to::<u64>();
        if config.effectively_enabled(now_ts) {
            return Err(FeeManagerError::currency_not_disabled(currency).into());
        }

        let budget = max_iterations.min(Self::MAX_PRUNE_ITERATIONS);
        let mut validators = self.token_validators[token].read()?;
        let take = (budget as usize).min(validators.len());
        let mut removed: u64 = 0;

        for _ in 0..take {
            let validator = validators
                .pop()
                .expect("loop bound matches validators.len()");
            self.validator_accepted_tokens[validator][token].write(false)?;

            let mut list = self.validator_token_list[validator].read()?;
            if let Some(pos) = list.iter().position(|t| *t == token) {
                list.swap_remove(pos);
                self.validator_token_list[validator].write(list)?;
            }
            self.emit_event(FeeManagerEvent::AcceptedTokenRemoved(
                IFeeManager::AcceptedTokenRemoved { validator, token },
            ))?;
            removed += 1;
        }

        self.token_validators[token].write(validators)?;
        config.last_pruned_at_block = current_block;
        self.supported_currencies[key].write(config)?;
        Ok(removed)
    }

    /// Genesis default escrow claim window: 365 days in seconds.
    pub const DEFAULT_ESCROW_CLAIM_WINDOW_SECS: u64 = 365 * 24 * 60 * 60;
    /// Sanity-bound minimum: 30 days.
    pub const MIN_ESCROW_CLAIM_WINDOW_SECS: u64 = 30 * 24 * 60 * 60;
    /// Sanity-bound maximum: 1825 days (~5 years).
    pub const MAX_ESCROW_CLAIM_WINDOW_SECS: u64 = 1825 * 24 * 60 * 60;

    pub fn escrow_claim_window(&self) -> Result<u64> {
        let raw = self.escrow_claim_window.read()?;
        Ok(if raw == 0 {
            Self::DEFAULT_ESCROW_CLAIM_WINDOW_SECS
        } else {
            raw
        })
    }

    pub fn foundation_escrow_address(&self) -> Result<Address> {
        self.foundation_escrow_address.read()
    }

    pub fn escrowed_fees_amount(&self, validator: Address, token: Address) -> Result<U256> {
        self.escrowed_fees[validator][token].read()
    }

    pub fn escrow_claim(&self, validator: Address) -> Result<ClaimRecord> {
        self.escrow_claims[validator].read()
    }

    pub fn set_escrow_claim_window(&mut self, sender: Address, new_window: u64) -> Result<()> {
        self.assert_governance(sender)?;
        if !(Self::MIN_ESCROW_CLAIM_WINDOW_SECS..=Self::MAX_ESCROW_CLAIM_WINDOW_SECS)
            .contains(&new_window)
        {
            return Err(FeeManagerError::escrow_claim_window_out_of_range(new_window).into());
        }
        let old = self.escrow_claim_window()?;
        self.escrow_claim_window.write(new_window)?;
        self.emit_event(FeeManagerEvent::EscrowClaimWindowChanged(
            IFeeManager::EscrowClaimWindowChanged {
                oldWindow: old,
                newWindow: new_window,
            },
        ))?;
        Ok(())
    }

    pub fn set_foundation_escrow_address(
        &mut self,
        sender: Address,
        new_address: Address,
    ) -> Result<()> {
        self.assert_governance(sender)?;
        if new_address.is_zero() {
            return Err(FeeManagerError::zero_address_foundation_escrow().into());
        }
        let old = self.foundation_escrow_address.read()?;
        self.foundation_escrow_address.write(new_address)?;
        self.emit_event(FeeManagerEvent::FoundationEscrowAddressChanged(
            IFeeManager::FoundationEscrowAddressChanged {
                oldAddress: old,
                newAddress: new_address,
            },
        ))?;
        Ok(())
    }

    /// Off-boards a validator: walks their accept-set, attempts direct delivery
    /// of each accumulated fee balance, escrows on failure, then clears the
    /// accept-set. Records a ClaimRecord stamping the deadline.
    pub fn offboard_validator(&mut self, sender: Address, validator: Address) -> Result<()> {
        self.assert_governance(sender)?;

        let existing = self.escrow_claims[validator].read()?;
        if existing.offboarded {
            return Err(FeeManagerError::validator_already_offboarded(validator).into());
        }

        let now_ts = self.storage.timestamp().saturating_to::<u64>();
        let claim_window = self.escrow_claim_window()?;
        let record = ClaimRecord::new(now_ts, claim_window);
        self.escrow_claims[validator].write(record.clone())?;

        let tokens = self.validator_token_list[validator].read()?;
        for token in &tokens {
            let amount = self.collected_fees[validator][*token].read()?;
            self.collected_fees[validator][*token].write(U256::ZERO)?;

            // Drop reverse-index + map flag for this validator/token pair.
            self.validator_accepted_tokens[validator][*token].write(false)?;
            let mut rv = self.token_validators[*token].read()?;
            if let Some(pos) = rv.iter().position(|v| *v == validator) {
                rv.swap_remove(pos);
                self.token_validators[*token].write(rv)?;
            }

            if amount.is_zero() {
                continue;
            }

            // Try direct MIP-20 transfer; on any error, escrow.
            let mut mip20 = MIP20Token::from_address(*token)?;
            let delivered = mip20
                .transfer(
                    self.address,
                    IMIP20::transferCall {
                        to: validator,
                        amount,
                    },
                )
                .is_ok();

            if delivered {
                self.emit_event(FeeManagerEvent::FeesOffboardDelivered(
                    IFeeManager::FeesOffboardDelivered {
                        validator,
                        token: *token,
                        amount,
                    },
                ))?;
            } else {
                let prior = self.escrowed_fees[validator][*token].read()?;
                let next = prior
                    .checked_add(amount)
                    .ok_or(MagnusPrecompileError::under_overflow())?;
                self.escrowed_fees[validator][*token].write(next)?;
                self.emit_event(FeeManagerEvent::FeesOffboardEscrowed(
                    IFeeManager::FeesOffboardEscrowed {
                        validator,
                        token: *token,
                        amount,
                    },
                ))?;
            }
        }

        // Clear accept-set list (map flags already zeroed above).
        self.validator_token_list[validator].write(Vec::new())?;

        self.emit_event(FeeManagerEvent::ValidatorOffboarded(
            IFeeManager::ValidatorOffboarded {
                validator,
                claimDeadline: record.claim_deadline,
            },
        ))?;
        Ok(())
    }

    /// Validator-org claims escrowed fees within the claim window. `sender` is the
    /// validator address — the caller must control it (multi-sig EIP-712 lands later).
    pub fn claim_escrowed_fees(
        &mut self,
        sender: Address,
        validator: Address,
        token: Address,
        recipient: Address,
    ) -> Result<U256> {
        if sender != validator {
            return Err(FeeManagerError::only_validator().into());
        }
        let record = self.escrow_claims[validator].read()?;
        if !record.offboarded {
            return Err(FeeManagerError::validator_not_offboarded(validator).into());
        }
        let now_ts = self.storage.timestamp().saturating_to::<u64>();
        if !record.within_claim_window(now_ts) {
            return Err(FeeManagerError::claim_window_expired(validator).into());
        }
        let amount = self.escrowed_fees[validator][token].read()?;
        if amount.is_zero() {
            return Err(FeeManagerError::no_escrowed_fees(validator, token).into());
        }

        self.escrowed_fees[validator][token].write(U256::ZERO)?;
        let mut mip20 = MIP20Token::from_address(token)?;
        mip20.transfer(
            self.address,
            IMIP20::transferCall {
                to: recipient,
                amount,
            },
        )?;

        self.emit_event(FeeManagerEvent::EscrowedFeesClaimed(
            IFeeManager::EscrowedFeesClaimed {
                validator,
                token,
                recipient,
                amount,
            },
        ))?;
        Ok(amount)
    }

    /// Governance sweeps a (validator, token) escrow entry to the foundation
    /// address after the claim window has expired.
    pub fn sweep_expired_escrow(
        &mut self,
        sender: Address,
        validator: Address,
        token: Address,
    ) -> Result<U256> {
        self.assert_governance(sender)?;

        let record = self.escrow_claims[validator].read()?;
        if !record.offboarded {
            return Err(FeeManagerError::validator_not_offboarded(validator).into());
        }
        let now_ts = self.storage.timestamp().saturating_to::<u64>();
        if !record.after_claim_window(now_ts) {
            return Err(FeeManagerError::claim_window_active(validator).into());
        }

        let amount = self.escrowed_fees[validator][token].read()?;
        if amount.is_zero() {
            return Err(FeeManagerError::no_escrowed_fees(validator, token).into());
        }
        let foundation = self.foundation_escrow_address.read()?;
        if foundation.is_zero() {
            return Err(FeeManagerError::zero_address_foundation_escrow().into());
        }

        self.escrowed_fees[validator][token].write(U256::ZERO)?;
        let mut mip20 = MIP20Token::from_address(token)?;
        mip20.transfer(
            self.address,
            IMIP20::transferCall {
                to: foundation,
                amount,
            },
        )?;

        self.emit_event(FeeManagerEvent::EscrowSwept(IFeeManager::EscrowSwept {
            validator,
            token,
            foundation,
            amount,
        }))?;
        Ok(amount)
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

#[cfg(test)]
mod currency_registry_tests {
    use super::*;
    use crate::{
        error::MagnusPrecompileError,
        storage::{ContractStorage, StorageCtx, hashmap::HashMapStorageProvider},
        test_util::MIP20Setup,
    };
    use magnus_chainspec::hardfork::MagnusHardfork;

    fn fee_manager_with_admin(admin: Address) -> Result<MipFeeManager> {
        let mut fee_manager = MipFeeManager::new();
        fee_manager.initialize()?;
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

            // Distinct salts -> distinct deterministic factory addresses.
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
    fn t4_settle_fee_direct_credits_when_validator_accepts() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new_with_spec(1, MagnusHardfork::T4);
        let admin = Address::random();
        let validator = Address::random();
        let fee_payer = Address::random();
        let beneficiary_other = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_with_admin_and_usd(admin)?;
            let token = MIP20Setup::create("USDC", "USDC", admin)
                .currency("USD")
                .apply()?;
            fm.add_accepted_token(validator, token.address(), beneficiary_other)?;

            // T4 path: validator accepts USDC -> direct credit.
            fm.collect_fee_post_tx(
                fee_payer,
                U256::from(100u64),
                U256::ZERO,
                token.address(),
                validator,
            )?;
            assert_eq!(
                fm.collected_fees[validator][token.address()].read()?,
                U256::from(100u64)
            );
            Ok(())
        })
    }

    #[test]
    fn t4_settle_fee_reverts_when_validator_does_not_accept() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new_with_spec(1, MagnusHardfork::T4);
        let admin = Address::random();
        let validator = Address::random();
        let fee_payer = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_with_admin_and_usd(admin)?;
            let token = MIP20Setup::create("USDC", "USDC", admin)
                .currency("USD")
                .apply()?;
            // Validator's accept-set is empty.

            let err = fm
                .collect_fee_post_tx(
                    fee_payer,
                    U256::from(100u64),
                    U256::ZERO,
                    token.address(),
                    validator,
                )
                .unwrap_err();
            assert_eq!(
                err,
                MagnusPrecompileError::FeeManagerError(FeeManagerError::fee_token_not_accepted(
                    validator,
                    token.address()
                ))
            );
            Ok(())
        })
    }

    #[test]
    fn is_accepted_by_any_validator_tracks_reverse_index() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let validator = Address::random();
        let beneficiary = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_with_admin_and_usd(admin)?;
            let token = MIP20Setup::create("USDC", "USDC", admin)
                .currency("USD")
                .apply()?;
            assert!(!fm.is_accepted_by_any_validator(token.address())?);
            fm.add_accepted_token(validator, token.address(), beneficiary)?;
            assert!(fm.is_accepted_by_any_validator(token.address())?);
            fm.remove_accepted_token(validator, token.address(), beneficiary)?;
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

    #[test]
    fn t4_set_user_token_reverts_with_api_removed() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new_with_spec(1, MagnusHardfork::T4);
        let admin = Address::random();
        let user = Address::random();
        StorageCtx::enter(&mut storage, || {
            let _fm = fee_manager_with_admin_and_usd(admin)?;
            let token = MIP20Setup::create("USDC", "USDC", admin)
                .currency("USD")
                .apply()?;

            let mut fm = MipFeeManager::new();
            let err = fm
                .set_user_token(
                    user,
                    IFeeManager::setUserTokenCall {
                        token: token.address(),
                    },
                )
                .unwrap_err();
            assert_eq!(
                err,
                MagnusPrecompileError::FeeManagerError(FeeManagerError::user_token_api_removed())
            );
            Ok(())
        })
    }

    #[test]
    fn t4_user_tokens_view_returns_zero_even_when_pre_t4_value_stored() -> eyre::Result<()> {
        // Pre-T4 storage may carry legacy values; T4 view must hide them.
        let user = Address::random();
        let pre_t4_token = Address::repeat_byte(0xAB);

        let mut storage = HashMapStorageProvider::new_with_spec(1, MagnusHardfork::T3);
        StorageCtx::enter(&mut storage, || {
            let mut fm = MipFeeManager::new();
            fm.user_tokens[user].write(pre_t4_token)?;
            assert_eq!(
                fm.user_tokens(IFeeManager::userTokensCall { user })?,
                pre_t4_token
            );
            Ok::<_, MagnusPrecompileError>(())
        })?;

        // Same provider, spec flipped to T4: legacy slot is hidden by the view.
        let mut storage = storage.with_spec(MagnusHardfork::T4);
        StorageCtx::enter(&mut storage, || {
            let fm = MipFeeManager::new();
            assert_eq!(
                fm.user_tokens(IFeeManager::userTokensCall { user })?,
                Address::ZERO
            );
            Ok(())
        })
    }

    #[test]
    fn disable_currency_starts_grace_period() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        storage.set_timestamp(U256::from(1_000u64));
        let admin = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_with_admin_and_usd(admin)?;

            fm.disable_currency(admin, "USD", 1_000)?;
            let cfg = fm.get_currency_config("USD")?;
            assert!(cfg.deprecating);
            assert!(cfg.enabled);
            assert_eq!(
                cfg.deprecation_activates_at,
                1_000 + MipFeeManager::DEFAULT_GRACE_PERIOD_SECS
            );
            assert!(cfg.in_grace_period(1_000));
            assert!(cfg.effectively_enabled(1_000));
            assert!(!cfg.effectively_enabled(cfg.deprecation_activates_at));
            Ok(())
        })
    }

    #[test]
    fn disable_currency_rejects_non_admin() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let intruder = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_with_admin_and_usd(admin)?;
            let err = fm.disable_currency(intruder, "USD", 0).unwrap_err();
            assert_eq!(
                err,
                MagnusPrecompileError::FeeManagerError(FeeManagerError::only_governance_admin(
                    intruder
                ))
            );
            Ok(())
        })
    }

    #[test]
    fn disable_currency_rejects_unregistered() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_with_admin(admin)?;
            let err = fm.disable_currency(admin, "EUR", 0).unwrap_err();
            assert_eq!(
                err,
                MagnusPrecompileError::FeeManagerError(FeeManagerError::currency_not_registered(
                    "EUR".into()
                ))
            );
            Ok(())
        })
    }

    #[test]
    fn disable_currency_rejects_double_disable() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_with_admin_and_usd(admin)?;
            fm.disable_currency(admin, "USD", 0)?;
            let err = fm.disable_currency(admin, "USD", 0).unwrap_err();
            assert_eq!(
                err,
                MagnusPrecompileError::FeeManagerError(
                    FeeManagerError::currency_already_deprecating("USD".into())
                )
            );
            Ok(())
        })
    }

    #[test]
    fn re_enable_clears_deprecation_flags() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_with_admin_and_usd(admin)?;
            fm.disable_currency(admin, "USD", 0)?;
            assert!(fm.get_currency_config("USD")?.deprecating);

            // Force re-enable: enable_currency rejects already-enabled, so flip the
            // flag down first by using enable after explicit disable. Here grace is
            // active but enabled is still true, so enable() returns AlreadyEnabled.
            // Simulate G6b emergency-disable by zeroing `enabled`, then re-enable.
            let key = currency_key("USD");
            let mut cfg = fm.supported_currencies[key].read()?;
            cfg.enabled = false;
            fm.supported_currencies[key].write(cfg)?;

            fm.enable_currency(admin, "USD", 100)?;
            let cfg = fm.get_currency_config("USD")?;
            assert!(cfg.enabled);
            assert!(!cfg.deprecating);
            assert_eq!(cfg.deprecation_activates_at, 0);
            Ok(())
        })
    }

    #[test]
    fn set_deprecation_grace_period_enforces_bounds() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_with_admin(admin)?;

            // Below the lower bound (1 hour - 1).
            let too_low = MipFeeManager::MIN_GRACE_PERIOD_SECS - 1;
            assert_eq!(
                fm.set_deprecation_grace_period(admin, too_low).unwrap_err(),
                MagnusPrecompileError::FeeManagerError(
                    FeeManagerError::grace_period_out_of_range(too_low)
                )
            );
            // Above the upper bound.
            let too_high = MipFeeManager::MAX_GRACE_PERIOD_SECS + 1;
            assert_eq!(
                fm.set_deprecation_grace_period(admin, too_high).unwrap_err(),
                MagnusPrecompileError::FeeManagerError(
                    FeeManagerError::grace_period_out_of_range(too_high)
                )
            );

            // Inside the range.
            fm.set_deprecation_grace_period(admin, 7 * 24 * 60 * 60)?;
            assert_eq!(fm.deprecation_grace_period()?, 7 * 24 * 60 * 60);
            Ok(())
        })
    }

    #[test]
    fn add_accepted_token_rejected_during_grace_period() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        storage.set_timestamp(U256::from(500u64));
        let admin = Address::random();
        let validator = Address::random();
        let beneficiary = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_with_admin_and_usd(admin)?;
            let token = MIP20Setup::create("USDC", "USDC", admin)
                .currency("USD")
                .apply()?;

            fm.disable_currency(admin, "USD", 500)?;

            let err = fm
                .add_accepted_token(validator, token.address(), beneficiary)
                .unwrap_err();
            let cfg = fm.get_currency_config("USD")?;
            assert_eq!(
                err,
                MagnusPrecompileError::FeeManagerError(FeeManagerError::currency_deprecating(
                    "USD".into(),
                    cfg.deprecation_activates_at
                ))
            );
            Ok(())
        })
    }

    #[test]
    fn emergency_disable_currency_flips_immediately() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_with_admin_and_usd(admin)?;
            assert!(fm.is_currency_enabled("USD")?);
            fm.emergency_disable_currency(admin, "USD")?;
            let cfg = fm.get_currency_config("USD")?;
            assert!(!cfg.enabled);
            assert!(!cfg.deprecating);
            assert!(!fm.is_currency_effectively_enabled("USD", 0)?);
            Ok(())
        })
    }

    #[test]
    fn emergency_disable_currency_clears_active_grace() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_with_admin_and_usd(admin)?;
            fm.disable_currency(admin, "USD", 0)?;
            assert!(fm.get_currency_config("USD")?.deprecating);
            fm.emergency_disable_currency(admin, "USD")?;
            let cfg = fm.get_currency_config("USD")?;
            assert!(!cfg.enabled);
            assert!(!cfg.deprecating);
            assert_eq!(cfg.deprecation_activates_at, 0);
            Ok(())
        })
    }

    #[test]
    fn emergency_disable_currency_rejects_already_disabled() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_with_admin(admin)?;
            fm.add_currency(admin, "USD", 0)?;
            // Never enabled.
            let err = fm.emergency_disable_currency(admin, "USD").unwrap_err();
            assert_eq!(
                err,
                MagnusPrecompileError::FeeManagerError(FeeManagerError::currency_disabled(
                    "USD".into()
                ))
            );
            Ok(())
        })
    }

    #[test]
    fn set_emergency_threshold_enforces_bounds() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_with_admin(admin)?;
            assert_eq!(
                fm.emergency_disable_threshold()?,
                MipFeeManager::DEFAULT_EMERGENCY_THRESHOLD
            );
            assert_eq!(
                fm.set_emergency_disable_threshold(admin, 5).unwrap_err(),
                MagnusPrecompileError::FeeManagerError(
                    FeeManagerError::emergency_threshold_out_of_range(5)
                )
            );
            assert_eq!(
                fm.set_emergency_disable_threshold(admin, 10).unwrap_err(),
                MagnusPrecompileError::FeeManagerError(
                    FeeManagerError::emergency_threshold_out_of_range(10)
                )
            );
            fm.set_emergency_disable_threshold(admin, 8)?;
            assert_eq!(fm.emergency_disable_threshold()?, 8);
            Ok(())
        })
    }

    #[test]
    fn prune_token_removes_validator_accept_set_entries() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let v1 = Address::random();
        let v2 = Address::random();
        let beneficiary = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_with_admin_and_usd(admin)?;
            let token = MIP20Setup::create("USDC", "USDC", admin)
                .currency("USD")
                .apply()?;
            fm.add_accepted_token(v1, token.address(), beneficiary)?;
            fm.add_accepted_token(v2, token.address(), beneficiary)?;
            assert!(fm.is_accepted_by_any_validator(token.address())?);

            // Prune is rejected while currency is still effectively enabled.
            assert_eq!(
                fm.prune_token(token.address(), 100, 1).unwrap_err(),
                MagnusPrecompileError::FeeManagerError(FeeManagerError::currency_not_disabled(
                    "USD".into()
                ))
            );

            fm.emergency_disable_currency(admin, "USD")?;

            // First call removes one entry (max_iter = 1, paginated).
            let removed = fm.prune_token(token.address(), 1, 1)?;
            assert_eq!(removed, 1);
            assert_eq!(fm.get_accepted_tokens(v1)?.len() + fm.get_accepted_tokens(v2)?.len(), 1);

            // Second call drains the rest.
            let removed = fm.prune_token(token.address(), 100, 2)?;
            assert_eq!(removed, 1);
            assert!(!fm.is_accepted_by_any_validator(token.address())?);
            assert_eq!(fm.get_accepted_tokens(v1)?.len(), 0);
            assert_eq!(fm.get_accepted_tokens(v2)?.len(), 0);

            // Third call is a no-op.
            assert_eq!(fm.prune_token(token.address(), 100, 3)?, 0);
            Ok(())
        })
    }

    #[test]
    fn prune_currency_stamps_block_when_disabled() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_with_admin_and_usd(admin)?;
            assert_eq!(
                fm.prune_currency("USD", 1, 0).unwrap_err(),
                MagnusPrecompileError::FeeManagerError(FeeManagerError::currency_not_disabled(
                    "USD".into()
                ))
            );
            fm.emergency_disable_currency(admin, "USD")?;
            fm.prune_currency("USD", 1, 42)?;
            assert_eq!(fm.get_currency_config("USD")?.last_pruned_at_block, 42);
            Ok(())
        })
    }

    fn fee_manager_for_offboarding(
        admin: Address,
        foundation: Address,
    ) -> Result<MipFeeManager> {
        let mut fm = fee_manager_with_admin_and_usd(admin)?;
        fm.set_foundation_escrow_address(admin, foundation)?;
        Ok(fm)
    }

    #[test]
    fn offboard_validator_delivers_when_transfer_succeeds() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        storage.set_timestamp(U256::from(10_000u64));
        let admin = Address::random();
        let validator = Address::random();
        let foundation = Address::random();
        let beneficiary = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_for_offboarding(admin, foundation)?;
            let token = MIP20Setup::create("USDC", "USDC", admin)
                .currency("USD")
                .with_issuer(admin)
                .with_mint(TIP_FEE_MANAGER_ADDRESS, U256::from(500u64))
                .apply()?;
            fm.add_accepted_token(validator, token.address(), beneficiary)?;
            fm.collected_fees[validator][token.address()].write(U256::from(500u64))?;

            fm.offboard_validator(admin, validator)?;

            // Direct delivery zeroed the ledger and credited the validator's
            // balance (token transfer succeeded since FeeManager was funded).
            assert_eq!(
                fm.collected_fees[validator][token.address()].read()?,
                U256::ZERO
            );
            assert_eq!(
                fm.escrowed_fees_amount(validator, token.address())?,
                U256::ZERO
            );
            assert_eq!(fm.get_accepted_tokens(validator)?.len(), 0);
            assert!(fm.escrow_claim(validator)?.offboarded);
            assert_eq!(
                fm.escrow_claim(validator)?.claim_deadline,
                10_000 + MipFeeManager::DEFAULT_ESCROW_CLAIM_WINDOW_SECS
            );
            Ok(())
        })
    }

    #[test]
    fn offboard_validator_escrows_when_transfer_fails() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        storage.set_timestamp(U256::from(10_000u64));
        let admin = Address::random();
        let validator = Address::random();
        let foundation = Address::random();
        let beneficiary = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_for_offboarding(admin, foundation)?;
            let token = MIP20Setup::create("USDC", "USDC", admin)
                .currency("USD")
                .apply()?;
            fm.add_accepted_token(validator, token.address(), beneficiary)?;

            // Stamp a non-zero ledger without funding the FeeManager. The transfer
            // attempt fails with InsufficientBalance and the amount is escrowed.
            fm.collected_fees[validator][token.address()].write(U256::from(750u64))?;

            fm.offboard_validator(admin, validator)?;

            assert_eq!(
                fm.collected_fees[validator][token.address()].read()?,
                U256::ZERO
            );
            assert_eq!(
                fm.escrowed_fees_amount(validator, token.address())?,
                U256::from(750u64)
            );
            Ok(())
        })
    }

    #[test]
    fn offboard_validator_rejects_double_offboard() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let validator = Address::random();
        let foundation = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_for_offboarding(admin, foundation)?;
            fm.offboard_validator(admin, validator)?;
            assert_eq!(
                fm.offboard_validator(admin, validator).unwrap_err(),
                MagnusPrecompileError::FeeManagerError(
                    FeeManagerError::validator_already_offboarded(validator)
                )
            );
            Ok(())
        })
    }

    #[test]
    fn claim_escrowed_fees_within_window_transfers_to_recipient() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        storage.set_timestamp(U256::from(0u64));
        let admin = Address::random();
        let validator = Address::random();
        let recipient = Address::random();
        let foundation = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_for_offboarding(admin, foundation)?;
            let token = MIP20Setup::create("USDC", "USDC", admin)
                .currency("USD")
                .with_issuer(admin)
                .with_mint(TIP_FEE_MANAGER_ADDRESS, U256::from(900u64))
                .apply()?;

            // Force escrow path by zeroing FeeManager balance after mint? Instead,
            // hand-write escrow state to drive the claim path.
            fm.escrow_claims[validator]
                .write(ClaimRecord::new(0, MipFeeManager::DEFAULT_ESCROW_CLAIM_WINDOW_SECS))?;
            fm.escrowed_fees[validator][token.address()].write(U256::from(900u64))?;

            let claimed =
                fm.claim_escrowed_fees(validator, validator, token.address(), recipient)?;
            assert_eq!(claimed, U256::from(900u64));
            assert_eq!(
                fm.escrowed_fees_amount(validator, token.address())?,
                U256::ZERO
            );
            assert_eq!(token.balances[recipient].read()?, U256::from(900u64));
            Ok(())
        })
    }

    #[test]
    fn claim_escrowed_fees_rejects_after_window() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        storage.set_timestamp(U256::from(0u64));
        let admin = Address::random();
        let validator = Address::random();
        let foundation = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_for_offboarding(admin, foundation)?;
            let token = MIP20Setup::create("USDC", "USDC", admin)
                .currency("USD")
                .apply()?;
            fm.escrow_claims[validator].write(ClaimRecord::new(0, 100))?;
            fm.escrowed_fees[validator][token.address()].write(U256::from(50u64))?;

            // Move past the deadline.
            drop(fm);
            Ok::<_, MagnusPrecompileError>(())
        })?;
        storage.set_timestamp(U256::from(101u64));
        StorageCtx::enter(&mut storage, || {
            let mut fm = MipFeeManager::new();
            let validator_token = Address::random();
            // Look up the escrowed token by reading directly — skip; instead,
            // assert error by calling claim with a dummy token address; behavior
            // depends only on the deadline check, not the token.
            let err = fm
                .claim_escrowed_fees(validator, validator, validator_token, validator)
                .unwrap_err();
            assert_eq!(
                err,
                MagnusPrecompileError::FeeManagerError(FeeManagerError::claim_window_expired(
                    validator
                ))
            );
            Ok(())
        })
    }

    #[test]
    fn claim_escrowed_fees_rejects_non_validator_caller() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        let validator = Address::random();
        let intruder = Address::random();
        let foundation = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_for_offboarding(admin, foundation)?;
            assert_eq!(
                fm.claim_escrowed_fees(intruder, validator, Address::random(), intruder)
                    .unwrap_err(),
                MagnusPrecompileError::FeeManagerError(FeeManagerError::only_validator())
            );
            Ok(())
        })
    }

    #[test]
    fn sweep_expired_escrow_after_window_sends_to_foundation() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        storage.set_timestamp(U256::from(0u64));
        let admin = Address::random();
        let validator = Address::random();
        let foundation = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_for_offboarding(admin, foundation)?;
            let token = MIP20Setup::create("USDC", "USDC", admin)
                .currency("USD")
                .with_issuer(admin)
                .with_mint(TIP_FEE_MANAGER_ADDRESS, U256::from(200u64))
                .apply()?;
            fm.escrow_claims[validator].write(ClaimRecord::new(0, 100))?;
            fm.escrowed_fees[validator][token.address()].write(U256::from(200u64))?;

            // Active window: sweep rejected.
            assert_eq!(
                fm.sweep_expired_escrow(admin, validator, token.address())
                    .unwrap_err(),
                MagnusPrecompileError::FeeManagerError(FeeManagerError::claim_window_active(
                    validator
                ))
            );
            Ok::<_, MagnusPrecompileError>(())
        })?;
        storage.set_timestamp(U256::from(101u64));
        StorageCtx::enter(&mut storage, || {
            let mut fm = MipFeeManager::new();
            // Find the token via stored escrow — tests stamp it explicitly. Use a
            // freshly minted token for this branch so we have a known address.
            // Re-derive by scanning would be brittle; instead, deploy a new escrow
            // entry with a new token to keep the test self-contained.
            let token2 = MIP20Setup::create("USDD", "USDD", admin)
                .currency("USD")
                .with_issuer(admin)
                .with_mint(TIP_FEE_MANAGER_ADDRESS, U256::from(123u64))
                .apply()?;
            fm.escrow_claims[validator].write(ClaimRecord::new(0, 100))?;
            fm.escrowed_fees[validator][token2.address()].write(U256::from(123u64))?;

            let swept = fm.sweep_expired_escrow(admin, validator, token2.address())?;
            assert_eq!(swept, U256::from(123u64));
            assert_eq!(
                fm.escrowed_fees_amount(validator, token2.address())?,
                U256::ZERO
            );
            assert_eq!(token2.balances[foundation].read()?, U256::from(123u64));
            Ok(())
        })
    }

    #[test]
    fn set_escrow_claim_window_enforces_bounds() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_with_admin(admin)?;
            assert_eq!(
                fm.set_escrow_claim_window(admin, MipFeeManager::MIN_ESCROW_CLAIM_WINDOW_SECS - 1)
                    .unwrap_err(),
                MagnusPrecompileError::FeeManagerError(
                    FeeManagerError::escrow_claim_window_out_of_range(
                        MipFeeManager::MIN_ESCROW_CLAIM_WINDOW_SECS - 1
                    )
                )
            );
            assert_eq!(
                fm.set_escrow_claim_window(admin, MipFeeManager::MAX_ESCROW_CLAIM_WINDOW_SECS + 1)
                    .unwrap_err(),
                MagnusPrecompileError::FeeManagerError(
                    FeeManagerError::escrow_claim_window_out_of_range(
                        MipFeeManager::MAX_ESCROW_CLAIM_WINDOW_SECS + 1
                    )
                )
            );
            fm.set_escrow_claim_window(admin, 90 * 24 * 60 * 60)?;
            assert_eq!(fm.escrow_claim_window()?, 90 * 24 * 60 * 60);
            Ok(())
        })
    }

    #[test]
    fn set_foundation_escrow_address_rejects_zero() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut fm = fee_manager_with_admin(admin)?;
            assert_eq!(
                fm.set_foundation_escrow_address(admin, Address::ZERO)
                    .unwrap_err(),
                MagnusPrecompileError::FeeManagerError(
                    FeeManagerError::zero_address_foundation_escrow()
                )
            );
            let target = Address::random();
            fm.set_foundation_escrow_address(admin, target)?;
            assert_eq!(fm.foundation_escrow_address()?, target);
            Ok(())
        })
    }
}
