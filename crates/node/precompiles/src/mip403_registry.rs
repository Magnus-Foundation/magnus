//! MIP403 Compliance Registry -- transfer authorization checks.
//!
//! Tokens can register compliance rules that are checked on every transfer.
//! This enables regulatory compliance (KYC/AML) at the protocol level.

use alloy_primitives::Address;
use crate::{addresses, error::Result};

pub struct MIP403Registry {
    pub address: Address,
}

impl MIP403Registry {
    pub fn new() -> Self {
        Self {
            address: addresses::MIP403_REGISTRY,
        }
    }

    /// Check if a transfer is authorized.
    ///
    /// Returns true if the transfer passes all compliance checks.
    /// Default: all transfers are authorized (no restrictions).
    pub fn is_transfer_authorized(
        &self,
        _token: Address,
        _from: Address,
        _to: Address,
    ) -> Result<bool> {
        // Default: permissionless. Compliance rules added via governance.
        Ok(true)
    }
}
