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

/// Compute the input amount for a rebalancing swap (validator -> user direction).
///
/// The rebalancer provides validator tokens to receive user tokens.
/// Rate: amount_in = ceil(amount_out * N / SCALE)
///
/// This is the inverse direction of fee swaps, allowing arbitrageurs
/// to rebalance pools after fee collection creates imbalance.
pub fn compute_rebalance_amount_in(amount_out: U256) -> Result<U256> {
    if amount_out.is_zero() {
        return Ok(U256::ZERO);
    }
    // Ceiling division: (amount_out * N + SCALE - 1) / SCALE
    let numerator = amount_out
        .checked_mul(N)
        .ok_or(MagnusPrecompileError::Overflow)?;
    let amount_in = numerator
        .checked_add(SCALE - U256::from(1))
        .and_then(|v| v.checked_div(SCALE))
        .ok_or(MagnusPrecompileError::Overflow)?;
    Ok(amount_in)
}

/// AMM pool with LP token tracking.
#[derive(Debug, Clone, Default)]
pub struct Pool {
    /// Reserve of user-side tokens.
    pub reserve_user: u128,
    /// Reserve of validator-side tokens.
    pub reserve_validator: u128,
    /// Total LP token supply.
    pub total_supply: u128,
}

impl Pool {
    /// Mint LP tokens for a validator-token-only deposit.
    ///
    /// First deposit: liquidity = amount/2 - MIN_LIQUIDITY (burned).
    /// Subsequent: liquidity = amount * total_supply / (V + n*U)
    /// where n = N/SCALE.
    pub fn mint(&mut self, amount_validator: u128) -> Result<u128> {
        if amount_validator == 0 {
            return Err(MagnusPrecompileError::InvalidInput(
                "zero deposit".into(),
            ));
        }

        let liquidity = if self.total_supply == 0 {
            let half = amount_validator / 2;
            if half <= MIN_LIQUIDITY.as_limbs()[0] as u128 {
                return Err(MagnusPrecompileError::InvalidInput(
                    "deposit too small".into(),
                ));
            }
            let liq = half - MIN_LIQUIDITY.as_limbs()[0] as u128;
            self.total_supply = MIN_LIQUIDITY.as_limbs()[0] as u128;
            self.reserve_validator = half;
            self.reserve_user = half;
            liq
        } else {
            let n_times_u = (self.reserve_user as u128)
                .checked_mul(N.as_limbs()[0] as u128)
                .and_then(|v| v.checked_div(SCALE.as_limbs()[0] as u128))
                .ok_or(MagnusPrecompileError::Overflow)?;
            let denominator = (self.reserve_validator as u128)
                .checked_add(n_times_u)
                .ok_or(MagnusPrecompileError::Overflow)?;
            let liq = (amount_validator as u128)
                .checked_mul(self.total_supply)
                .and_then(|v| v.checked_div(denominator))
                .ok_or(MagnusPrecompileError::Overflow)?;
            self.reserve_validator = self.reserve_validator
                .checked_add(amount_validator)
                .ok_or(MagnusPrecompileError::Overflow)?;
            liq
        };

        self.total_supply = self.total_supply
            .checked_add(liquidity)
            .ok_or(MagnusPrecompileError::Overflow)?;
        Ok(liquidity)
    }

    /// Burn LP tokens for pro-rata share of reserves.
    pub fn burn(&mut self, liquidity: u128) -> Result<(u128, u128)> {
        if liquidity == 0 || liquidity > self.total_supply {
            return Err(MagnusPrecompileError::InvalidInput(
                "invalid liquidity amount".into(),
            ));
        }

        let amount_user = (liquidity as u128)
            .checked_mul(self.reserve_user)
            .and_then(|v| v.checked_div(self.total_supply))
            .ok_or(MagnusPrecompileError::Overflow)?;

        let amount_validator = (liquidity as u128)
            .checked_mul(self.reserve_validator)
            .and_then(|v| v.checked_div(self.total_supply))
            .ok_or(MagnusPrecompileError::Overflow)?;

        self.reserve_user -= amount_user;
        self.reserve_validator -= amount_validator;
        self.total_supply -= liquidity;

        Ok((amount_user, amount_validator))
    }
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

    #[test]
    fn rebalance_amount_in_correct() {
        let result = compute_rebalance_amount_in(U256::from(10000)).unwrap();
        assert_eq!(result, U256::from(9985));
    }

    #[test]
    fn rebalance_amount_in_rounds_up() {
        let result = compute_rebalance_amount_in(U256::from(3)).unwrap();
        assert_eq!(result, U256::from(3));
    }

    #[test]
    fn rebalance_zero() {
        let result = compute_rebalance_amount_in(U256::ZERO).unwrap();
        assert_eq!(result, U256::ZERO);
    }

    #[test]
    fn pool_first_mint() {
        let mut pool = Pool::default();
        let liq = pool.mint(10000).unwrap();
        assert_eq!(liq, 4000);
        assert_eq!(pool.total_supply, 4000 + 1000);
        assert_eq!(pool.reserve_user, 5000);
        assert_eq!(pool.reserve_validator, 5000);
    }

    #[test]
    fn pool_first_mint_too_small() {
        let mut pool = Pool::default();
        let result = pool.mint(1000);
        assert!(result.is_err());
    }

    #[test]
    fn pool_burn_proportional() {
        let mut pool = Pool::default();
        let liq = pool.mint(10000).unwrap();
        let (user_out, val_out) = pool.burn(liq).unwrap();
        assert!(user_out > 0);
        assert!(val_out > 0);
    }

    #[test]
    fn pool_burn_zero_rejected() {
        let mut pool = Pool::default();
        pool.mint(10000).unwrap();
        assert!(pool.burn(0).is_err());
    }
}
