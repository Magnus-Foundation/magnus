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

#[test]
fn oracle_cross_currency_conversion() {
    use magnus_precompiles::fee_manager::convert_via_oracle;

    let mut oracle = OracleRegistry::new();
    let reporter = Address::with_last_byte(1);
    let vnd_token = Address::with_last_byte(10);
    let usd_token = Address::with_last_byte(20);

    oracle.add_reporter(reporter);
    // 1 VND = 0.00004 USD (25000 VND per USD)
    // In 18-decimal fixed-point: 0.00004 * 10^18 = 4 * 10^13
    let rate = U256::from(4u64) * U256::from(10u64.pow(13));
    oracle.report(reporter, vnd_token, usd_token, rate, 1000).unwrap();

    // Convert 25,000,000 VND -> USD
    let amount_vnd = U256::from(25_000_000u64);
    let amount_usd = convert_via_oracle(&mut oracle, amount_vnd, vnd_token, usd_token, 1000).unwrap();
    // 25_000_000 * 4*10^13 / 10^18 = 1000
    assert_eq!(amount_usd, U256::from(1000u64));
}

#[test]
fn fee_amm_rebalance_swap() {
    let amount_out = U256::from(10000);
    let amount_in = fee_amm::compute_rebalance_amount_in(amount_out).unwrap();
    // ceil(10000 * 9985 / 10000) = 9985
    assert_eq!(amount_in, U256::from(9985));
}

#[test]
fn full_fee_flow_with_refund() {
    // Scenario: user pays in VND, validator wants USD
    let gas_limit = 100_000u64;
    let max_fee_per_gas = 1_000_000_000u128; // 1 gwei

    let max_fee = fee_handler::calculate_max_fee(gas_limit, max_fee_per_gas);
    assert_eq!(max_fee, U256::from(100_000_000_000_000u64));

    // TX uses 60% of gas
    let gas_used = 60_000u64;
    let actual_fee = fee_handler::calculate_actual_fee(gas_used, max_fee_per_gas);
    let refund = fee_handler::calculate_refund(max_fee, actual_fee);

    // Refund = 40% of max_fee
    assert_eq!(refund, U256::from(40_000_000_000_000u64));

    // AMM swap on the actual fee amount
    let swap_output = fee_amm::compute_amount_out(actual_fee).unwrap();
    // 60_000_000_000_000 * 9970 / 10000 = 59_820_000_000_000
    assert_eq!(swap_output, U256::from(59_820_000_000_000u64));
}
