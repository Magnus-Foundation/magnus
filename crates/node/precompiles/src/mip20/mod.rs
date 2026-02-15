//! MIP20 token -- ERC-20 compatible native token precompile.
//!
//! Each MIP20 lives at a deterministic address: 0x20C0...{token_id}.
//! Supports role-based access (admin, minter, burner, pauser) and
//! per-transfer compliance checks via MIP403.

pub mod roles;

use alloy_primitives::{Address, U256};
use crate::{
    addresses::MIP20_PREFIX,
    error::{MagnusPrecompileError, Result},
    storage::Mapping,
    storage::mapping::StorageKey,
};

/// MIP20 address prefix check.
pub fn is_mip20_prefix(addr: Address) -> bool {
    addr.as_slice()[..12] == MIP20_PREFIX
}

/// MIP20 token state stored at the token's precompile address.
#[derive(Debug)]
#[allow(dead_code)]
pub struct MIP20Token {
    /// The deterministic precompile address for this token.
    pub address: Address,
    // Storage slot 0: balances mapping
    balances: Mapping<Address, U256>,
    // Storage slot 1: allowances mapping (owner -> spender -> amount)
    // Implemented as nested: slot = keccak(spender, keccak(owner, 1))
    allowances_base: U256,
    // Storage slot 2: total supply
    total_supply_slot: U256,
    // Storage slot 3: roles mapping
    roles_base: U256,
    // Storage slot 4: metadata (name, symbol, decimals, currency)
    metadata_base: U256,
    // Storage slot 5: transfer policy ID (MIP403). Default: 1 (always-allow).
    transfer_policy_slot: U256,
    // Storage slot 6: supply cap (0 = unlimited).
    supply_cap_slot: U256,
}

impl MIP20Token {
    /// Create a MIP20 token instance from its address.
    pub fn from_address(address: Address) -> Result<Self> {
        if !is_mip20_prefix(address) {
            return Err(MagnusPrecompileError::InvalidInput(
                "not a MIP20 address".into(),
            ));
        }
        Ok(Self {
            address,
            balances: Mapping::new(address, U256::from(0)),
            allowances_base: U256::from(1),
            total_supply_slot: U256::from(2),
            roles_base: U256::from(3),
            metadata_base: U256::from(4),
            transfer_policy_slot: U256::from(5),
            supply_cap_slot: U256::from(6),
        })
    }

    /// Check recipient is valid (not zero, not another MIP20 address).
    fn check_recipient(&self, to: Address) -> Result<()> {
        if to.is_zero() || crate::addresses::is_mip20_prefix(to) {
            return Err(MagnusPrecompileError::InvalidInput(
                "invalid recipient".into(),
            ));
        }
        Ok(())
    }

    /// Get the transfer policy ID for this token. Default: 1 (always-allow).
    pub fn transfer_policy_id(&self) -> u64 {
        let val = crate::storage::sload(self.address, self.transfer_policy_slot);
        let id = val.as_limbs()[0];
        if id == 0 { 1 } else { id } // Default to policy 1 (always-allow)
    }

    /// Set the transfer policy ID (admin-only in production).
    pub fn set_transfer_policy_id(&mut self, policy_id: u64) {
        crate::storage::sstore(
            self.address,
            self.transfer_policy_slot,
            U256::from(policy_id),
        );
    }

    /// Get the supply cap. Returns U256::MAX if no cap is set.
    pub fn supply_cap(&self) -> U256 {
        let val = crate::storage::sload(self.address, self.supply_cap_slot);
        if val.is_zero() { U256::MAX } else { val }
    }

    /// Set the supply cap. Must be >= current total supply.
    pub fn set_supply_cap(&mut self, new_cap: U256) -> Result<()> {
        let current_supply = self.total_supply();
        if new_cap < current_supply {
            return Err(MagnusPrecompileError::InvalidInput(
                "cap below current supply".into(),
            ));
        }
        crate::storage::sstore(self.address, self.supply_cap_slot, new_cap);
        Ok(())
    }

    /// Get balance of an account.
    pub fn balance_of(&self, account: Address) -> U256 {
        self.balances.read(&account)
    }

    /// Transfer tokens. Returns Ok(true) on success.
    pub fn transfer(
        &mut self,
        from: Address,
        to: Address,
        amount: U256,
    ) -> Result<bool> {
        self.check_recipient(to)?;
        let from_balance = self.balances.read(&from);
        if from_balance < amount {
            return Err(MagnusPrecompileError::InsufficientBalance);
        }
        self.balances.write(&from, from_balance - amount);
        let to_balance = self.balances.read(&to);
        self.balances.write(
            &to,
            to_balance.checked_add(amount)
                .ok_or(MagnusPrecompileError::Overflow)?,
        );
        Ok(true)
    }

    /// Mint new tokens to an account. Caller must have minter role.
    pub fn mint(&mut self, to: Address, amount: U256) -> Result<bool> {
        // Check supply cap
        let cap = self.supply_cap();
        let current = self.total_supply();
        let new_supply = current.checked_add(amount)
            .ok_or(MagnusPrecompileError::Overflow)?;
        if new_supply > cap {
            return Err(MagnusPrecompileError::InvalidInput(
                "would exceed supply cap".into(),
            ));
        }
        let balance = self.balances.read(&to);
        self.balances.write(
            &to,
            balance.checked_add(amount)
                .ok_or(MagnusPrecompileError::Overflow)?,
        );
        // Update total supply
        let supply = crate::storage::sload(self.address, self.total_supply_slot);
        crate::storage::sstore(
            self.address,
            self.total_supply_slot,
            supply.checked_add(amount)
                .ok_or(MagnusPrecompileError::Overflow)?,
        );
        Ok(true)
    }

    /// Burn tokens from an account.
    pub fn burn(&mut self, from: Address, amount: U256) -> Result<bool> {
        let balance = self.balances.read(&from);
        if balance < amount {
            return Err(MagnusPrecompileError::InsufficientBalance);
        }
        self.balances.write(&from, balance - amount);
        let supply = crate::storage::sload(self.address, self.total_supply_slot);
        crate::storage::sstore(self.address, self.total_supply_slot, supply - amount);
        Ok(true)
    }

    /// Total supply.
    pub fn total_supply(&self) -> U256 {
        crate::storage::sload(self.address, self.total_supply_slot)
    }

    /// Transfer tokens on behalf of owner (requires allowance).
    /// Used by FeeManager to deduct gas fees.
    pub fn transfer_from(
        &mut self,
        spender: Address,
        from: Address,
        to: Address,
        amount: U256,
    ) -> Result<bool> {
        self.check_recipient(to)?;
        // Check and update allowance
        let allowance = self.allowance(from, spender);
        if allowance < amount && allowance != U256::MAX {
            return Err(MagnusPrecompileError::Unauthorized(
                "insufficient allowance".into(),
            ));
        }
        if allowance != U256::MAX {
            self.set_allowance(from, spender, allowance - amount);
        }
        self.transfer(from, to, amount)
    }

    /// Get allowance.
    pub fn allowance(&self, owner: Address, spender: Address) -> U256 {
        let slot = self.allowance_slot(owner, spender);
        crate::storage::sload(self.address, slot)
    }

    /// Approve spender.
    pub fn approve(
        &mut self,
        owner: Address,
        spender: Address,
        amount: U256,
    ) -> Result<bool> {
        self.set_allowance(owner, spender, amount);
        Ok(true)
    }

    fn set_allowance(&mut self, owner: Address, spender: Address, amount: U256) {
        let slot = self.allowance_slot(owner, spender);
        crate::storage::sstore(self.address, slot, amount);
    }

    /// Transfer tokens with a 32-byte memo.
    ///
    /// The memo is not stored on-chain -- it is emitted in event logs only.
    /// Used for payment references, invoice IDs, and transaction notes.
    pub fn transfer_with_memo(
        &mut self,
        from: Address,
        to: Address,
        amount: U256,
        _memo: [u8; 32],
    ) -> Result<bool> {
        self.check_recipient(to)?;
        self.transfer(from, to, amount)
        // In production, emit TransferWithMemo event with memo
    }

    /// Mint tokens with a 32-byte memo.
    pub fn mint_with_memo(
        &mut self,
        to: Address,
        amount: U256,
        _memo: [u8; 32],
    ) -> Result<bool> {
        self.mint(to, amount)
        // In production, emit TransferWithMemo event from Address::ZERO
    }

    /// Burn tokens with a 32-byte memo.
    pub fn burn_with_memo(
        &mut self,
        from: Address,
        amount: U256,
        _memo: [u8; 32],
    ) -> Result<bool> {
        self.burn(from, amount)
        // In production, emit TransferWithMemo event to Address::ZERO
    }

    fn allowance_slot(&self, owner: Address, spender: Address) -> U256 {
        // Nested mapping: keccak(spender, keccak(owner, base_slot))
        let inner = crate::storage::mapping::mapping_slot(
            &owner.to_slot_bytes(),
            &self.allowances_base,
        );
        crate::storage::mapping::mapping_slot(
            &spender.to_slot_bytes(),
            &inner,
        )
    }
}
