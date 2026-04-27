//! No-op stub of the legacy AMM liquidity cache.
//!
//! The AMM was deleted in G3b. The pool no longer pre-validates fee-swap
//! liquidity — settlement at T4 is direct-credit-or-revert, which the
//! handler enforces via the validator accept-set. This stub keeps the public
//! API surface so dependent crates compile without a full pipeline rewrite;
//! every method is intentionally permissive.

use std::sync::Arc;

use alloy_primitives::{Address, U256};
use parking_lot::RwLock;
use reth_primitives_traits::SealedHeader;
use reth_provider::{ExecutionOutcome, ProviderResult};
use magnus_primitives::{MagnusHeader, MagnusReceipt};

#[derive(Debug, Clone, Default)]
pub struct AmmLiquidityCache {
    _inner: Arc<RwLock<()>>,
}

impl AmmLiquidityCache {
    pub fn new<P>(_provider: P) -> ProviderResult<Self> {
        Ok(Self::default())
    }

    pub fn with_unique_tokens(_tokens: Vec<Address>) -> Self {
        Self::default()
    }

    pub fn repopulate<P>(&self, _provider: P) -> ProviderResult<()> {
        Ok(())
    }

    pub fn on_new_state(&self, _outcome: &ExecutionOutcome<MagnusReceipt>) {}

    pub fn on_new_blocks<P, I, H>(&self, _blocks: I, _client: P) -> ProviderResult<()>
    where
        I: IntoIterator<Item = H>,
    {
        let _ = SealedHeader::<MagnusHeader>::default;
        Ok(())
    }

    pub fn is_active_validator(&self, _validator: &Address) -> bool {
        true
    }

    pub fn is_active_validator_token(&self, _token: &Address) -> bool {
        true
    }

    pub fn track_tokens(&self, _tokens: &[Address]) -> bool {
        false
    }

    pub fn has_enough_liquidity<P>(
        &self,
        _user_token: Address,
        _cost: U256,
        _provider: &mut P,
    ) -> ProviderResult<bool> {
        Ok(true)
    }
}
