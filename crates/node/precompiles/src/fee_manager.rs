//! Fee Manager -- collects, refunds, and swaps gas fees in stablecoins.
//!
//! Key difference from Tempo: NO currency restriction. Any MIP20 token
//! can be a fee token (USD, VND, EUR, NGN, CBDC, etc.).
//!
//! Flow:
//! 1. Pre-tx: collect_fee_pre_tx() deducts max_fee from user in their chosen token
//! 2. EVM executes the transaction
//! 3. Post-tx: collect_fee_post_tx() refunds unused gas, swaps if needed
//!
//! Cross-currency conversion uses OracleRegistry rates when user and
//! validator tokens have different denominations.

use alloy_primitives::{Address, U256};
use crate::{
    addresses,
    error::{MagnusPrecompileError, Result},
    fee_amm,
    storage::Mapping,
    mip20::MIP20Token,
    mip20_factory::MIP20Factory,
};

/// Fee Manager state.
#[derive(Debug)]
pub struct FeeManager {
    /// The precompile address of this FeeManager.
    pub address: Address,
    /// Validator preferred token: validator_address -> token_address
    validator_tokens: Mapping<Address, Address>,
    /// User preferred token: user_address -> token_address
    user_tokens: Mapping<Address, Address>,
    /// Accumulated fees base slot (for nested mapping: validator -> token -> amount)
    #[allow(dead_code)]
    collected_fees_base: U256,
}

/// Default fee token used when no preference is set.
/// This should be configured at chain genesis.
pub const DEFAULT_FEE_TOKEN: Address = addresses::MIP20_FACTORY; // Placeholder

impl FeeManager {
    /// Create a new FeeManager instance.
    pub fn new() -> Self {
        Self {
            address: addresses::FEE_MANAGER,
            validator_tokens: Mapping::new(addresses::FEE_MANAGER, U256::from(0)),
            user_tokens: Mapping::new(addresses::FEE_MANAGER, U256::from(1)),
            collected_fees_base: U256::from(2),
        }
    }

    /// Get the validator's preferred fee token.
    pub fn get_validator_token(&self, validator: Address) -> Address {
        let val = self.validator_tokens.read_address(&validator);
        if val == Address::ZERO {
            DEFAULT_FEE_TOKEN
        } else {
            val
        }
    }

    /// Set user's preferred fee token.
    /// NO currency validation -- any registered MIP20 is accepted.
    pub fn set_user_token(&mut self, user: Address, token: Address) -> Result<()> {
        // Only validate that token is a deployed MIP20
        if !MIP20Factory::new().is_mip20(token)? {
            return Err(MagnusPrecompileError::InvalidInput(
                "token not a registered MIP20".into(),
            ));
        }
        self.user_tokens.write_address(&user, token);
        Ok(())
    }

    /// Pre-transaction fee collection.
    ///
    /// 1. Deducts max_fee from user in their chosen token
    /// 2. Checks liquidity if cross-currency swap needed
    /// 3. Returns the user's fee token address
    pub fn collect_fee_pre_tx(
        &mut self,
        fee_payer: Address,
        user_token: Address,
        max_amount: U256,
        beneficiary: Address,
    ) -> Result<Address> {
        let validator_token = self.get_validator_token(beneficiary);

        // Transfer max_amount from user to FeeManager
        let mut mip20 = MIP20Token::from_address(user_token)?;
        mip20.transfer(fee_payer, self.address, max_amount)?;

        // If cross-currency, check AMM has sufficient liquidity
        if user_token != validator_token {
            let _amount_out = fee_amm::compute_amount_out(max_amount)?;
            // TODO: check pool reserves >= amount_out
        }

        Ok(user_token)
    }

    /// Post-transaction fee settlement.
    ///
    /// 1. Refunds unused gas to user
    /// 2. Swaps fee if cross-currency
    /// 3. Accumulates fees for validator
    pub fn collect_fee_post_tx(
        &mut self,
        fee_payer: Address,
        actual_spending: U256,
        refund_amount: U256,
        fee_token: Address,
        beneficiary: Address,
    ) -> Result<()> {
        // Refund unused tokens to user
        if !refund_amount.is_zero() {
            let mut mip20 = MIP20Token::from_address(fee_token)?;
            mip20.transfer(self.address, fee_payer, refund_amount)?;
        }

        let validator_token = self.get_validator_token(beneficiary);

        let _fee_amount = if fee_token != validator_token && !actual_spending.is_zero() {
            // Cross-currency: compute swap output
            fee_amm::compute_amount_out(actual_spending)?
        } else {
            actual_spending
        };

        // Track accumulated fees (simplified -- full impl uses nested mapping)
        // TODO: implement nested mapping for collected_fees[validator][token]

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fee_manager_new() {
        let fm = FeeManager::new();
        assert_eq!(fm.address, addresses::FEE_MANAGER);
    }
}
