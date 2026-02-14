//! Integration test: end-to-end stablecoin gas fee flow.
//!
//! Scenario:
//! 1. User has VND stablecoin tokens
//! 2. Validator prefers USD stablecoin tokens
//! 3. User submits tx with fee_token = VND
//! 4. FeeManager deducts max_fee in VND
//! 5. After execution, FeeManager refunds unused VND
//! 6. FeeManager swaps actual_fee VND -> USD via AMM
//! 7. Validator receives fees in USD

use magnus_precompiles::{
    fee_amm,
    fee_manager::FeeManager,
    oracle_registry::OracleRegistry,
};
use magnus_executor::fee_handler;
use alloy_primitives::{Address, U256};

#[test]
fn fee_collection_same_currency() {
    let _fm = FeeManager::new();
    // When user and validator use same token, no swap needed
    let max_fee = fee_handler::calculate_max_fee(21000, 1_000_000_000);
    let actual_fee = fee_handler::calculate_actual_fee(21000, 500_000_000);
    let refund = fee_handler::calculate_refund(max_fee, actual_fee);
    assert!(refund > U256::ZERO);
}

#[test]
fn oracle_rate_lookup() {
    let mut oracle = OracleRegistry::new();
    let reporter = Address::with_last_byte(1);
    let vnd_token = Address::with_last_byte(10);
    let usd_token = Address::with_last_byte(20);

    oracle.add_reporter(reporter);
    oracle.report(reporter, vnd_token, usd_token, U256::from(25_500), 1000)
        .unwrap();

    let rate = oracle.get_rate(vnd_token, usd_token, 1000).unwrap();
    assert_eq!(rate, U256::from(25_500));
}

#[test]
fn fee_amm_swap_output() {
    let amount = U256::from(25_500_000); // 25.5M VND
    let output = fee_amm::compute_amount_out(amount).unwrap();
    // 25_500_000 * 9970 / 10000 = 25_423_500
    assert_eq!(output, U256::from(25_423_500));
}
