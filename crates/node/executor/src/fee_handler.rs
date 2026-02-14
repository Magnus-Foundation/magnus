//! Fee handler hooks for stablecoin gas fee collection.
//!
//! These hooks are called by the EVM executor around each transaction:
//!
//! 1. `deduct_fee_pre_tx` -- before EVM execution
//!    - Determines user's fee token (from tx.fee_token or user preference)
//!    - Deducts max_fee from user via FeeManager
//!    - Returns fee context for post-tx processing
//!
//! 2. `refund_fee_post_tx` -- after EVM execution
//!    - Calculates actual gas used
//!    - Refunds unused gas to user
//!    - Executes cross-currency swap if needed
//!    - Accumulates fees for validator

use alloy_primitives::{Address, U256};

/// Fee context passed between pre-tx and post-tx hooks.
#[derive(Debug, Clone)]
pub struct FeeContext {
    /// Who is paying the fee.
    pub fee_payer: Address,
    /// Token used for fee payment.
    pub fee_token: Address,
    /// Maximum fee amount deducted pre-tx.
    pub max_fee: U256,
    /// Block beneficiary (validator).
    pub beneficiary: Address,
}

/// Calculate the maximum fee for a transaction.
///
/// max_fee = gas_limit * max_fee_per_gas
///
/// The max_fee_per_gas is denominated in "abstract gas units" which are
/// then converted to the fee token amount using the oracle rate.
pub fn calculate_max_fee(gas_limit: u64, max_fee_per_gas: u128) -> U256 {
    U256::from(gas_limit) * U256::from(max_fee_per_gas)
}

/// Calculate the actual fee after execution.
///
/// actual_fee = gas_used * effective_gas_price
pub fn calculate_actual_fee(gas_used: u64, effective_gas_price: u128) -> U256 {
    U256::from(gas_used) * U256::from(effective_gas_price)
}

/// Calculate the refund amount.
pub fn calculate_refund(max_fee: U256, actual_fee: U256) -> U256 {
    max_fee.saturating_sub(actual_fee)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_fee_calculation() {
        let max = calculate_max_fee(21000, 1_000_000_000); // 21k gas * 1 gwei
        assert_eq!(max, U256::from(21_000_000_000_000u64));
    }

    #[test]
    fn actual_fee_calculation() {
        let actual = calculate_actual_fee(21000, 500_000_000); // 21k * 0.5 gwei
        assert_eq!(actual, U256::from(10_500_000_000_000u64));
    }

    #[test]
    fn refund_calculation() {
        let max = U256::from(100);
        let actual = U256::from(60);
        assert_eq!(calculate_refund(max, actual), U256::from(40));
    }

    #[test]
    fn refund_saturates_at_zero() {
        let max = U256::from(50);
        let actual = U256::from(100);
        assert_eq!(calculate_refund(max, actual), U256::ZERO);
    }
}
