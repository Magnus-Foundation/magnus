//! Fee AMM -- constant-rate swap for cross-currency gas fee conversion.
//!
//! When a user pays gas in token A but the validator wants token B,
//! the FeeManager swaps through this AMM at a fixed rate.
//!
//! Constants (from Tempo):
//! - M = 9970/10000 (0.30% fee on fee swaps)
//! - N = 9985/10000 (0.15% fee on rebalance swaps)
//! - MIN_LIQUIDITY = 1000

use alloy_primitives::{Address, B256, U256, keccak256, uint};
use crate::error::{MagnusPrecompileError, Result};

/// Fee swap rate numerator: 0.30% fee.
pub const M: U256 = uint!(9970_U256);
/// Rebalance swap rate numerator: 0.15% fee.
pub const N: U256 = uint!(9985_U256);
/// Rate denominator.
pub const SCALE: U256 = uint!(10000_U256);
/// Minimum liquidity for a pool.
pub const MIN_LIQUIDITY: U256 = uint!(1000_U256);

/// Compute output amount for a fee swap at fixed rate M/SCALE.
#[inline]
pub fn compute_amount_out(amount_in: U256) -> Result<U256> {
    amount_in
        .checked_mul(M)
        .map(|product| product / SCALE)
        .ok_or(MagnusPrecompileError::Overflow)
}

/// Pool reserves for a token pair.
#[derive(Debug, Clone, Default)]
pub struct Pool {
    /// Reserve amount of token A.
    pub reserve_a: u128,
    /// Reserve amount of token B.
    pub reserve_b: u128,
}

/// Compute the pool ID for a token pair.
pub fn pool_id(token_a: Address, token_b: Address) -> B256 {
    let mut data = Vec::with_capacity(40);
    data.extend_from_slice(token_a.as_slice());
    data.extend_from_slice(token_b.as_slice());
    keccak256(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_amount_out_correct() {
        let amount = U256::from(10000);
        let out = compute_amount_out(amount).unwrap();
        // 10000 * 9970 / 10000 = 9970
        assert_eq!(out, U256::from(9970));
    }

    #[test]
    fn compute_amount_out_small() {
        let amount = U256::from(100);
        let out = compute_amount_out(amount).unwrap();
        // 100 * 9970 / 10000 = 99 (integer division)
        assert_eq!(out, U256::from(99));
    }

    #[test]
    fn pool_id_deterministic() {
        let a = Address::ZERO;
        let b = Address::with_last_byte(1);
        let id1 = pool_id(a, b);
        let id2 = pool_id(a, b);
        assert_eq!(id1, id2);
    }

    #[test]
    fn pool_id_order_matters() {
        let a = Address::ZERO;
        let b = Address::with_last_byte(1);
        let id_ab = pool_id(a, b);
        let id_ba = pool_id(b, a);
        assert_ne!(id_ab, id_ba);
    }
}
