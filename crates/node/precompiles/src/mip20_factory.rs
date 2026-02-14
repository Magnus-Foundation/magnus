//! MIP20 Factory -- deploys and tracks MIP20 tokens.

use alloy_primitives::{Address, U256};
use crate::{
    addresses,
    error::Result,
    storage::Mapping,
};

/// Factory for deploying and tracking MIP20 tokens.
#[derive(Debug)]
pub struct MIP20Factory {
    #[allow(dead_code)]
    address: Address,
    /// Mapping from token address -> bool (is_deployed)
    deployed: Mapping<Address, U256>,
}

impl MIP20Factory {
    /// Create a new MIP20 factory instance.
    pub fn new() -> Self {
        Self {
            address: addresses::MIP20_FACTORY,
            deployed: Mapping::new(addresses::MIP20_FACTORY, U256::from(0)),
        }
    }

    /// Check if a token address is a deployed MIP20.
    pub fn is_mip20(&self, token: Address) -> Result<bool> {
        Ok(self.deployed.read(&token) != U256::ZERO)
    }

    /// Register a new MIP20 token (called during deployment).
    pub fn register(&mut self, token: Address) -> Result<()> {
        self.deployed.write(&token, U256::from(1));
        Ok(())
    }
}
