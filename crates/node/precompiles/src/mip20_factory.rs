//! MIP20 Factory -- deploys and tracks MIP20 tokens.

use alloy_primitives::{Address, B256, U256};
use crate::{
    addresses,
    error::{MagnusPrecompileError, Result},
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

    /// Create a new MIP20 token with deterministic address.
    ///
    /// Address = MIP20_PREFIX || keccak256(creator, salt)[..8]
    pub fn create_token(
        &mut self,
        creator: Address,
        salt: B256,
    ) -> Result<Address> {
        use alloy_primitives::keccak256;

        // Compute deterministic address
        let mut hash_input = [0u8; 52]; // 20 bytes creator + 32 bytes salt
        hash_input[..20].copy_from_slice(creator.as_slice());
        hash_input[20..52].copy_from_slice(salt.as_slice());
        let hash = keccak256(hash_input);

        // Address = prefix (12 bytes) || hash[..8] (8 bytes) padded to 20 bytes
        let mut addr_bytes = [0u8; 20];
        addr_bytes[..12].copy_from_slice(&crate::addresses::MIP20_PREFIX);
        addr_bytes[12..20].copy_from_slice(&hash[..8]);
        let token_addr = Address::from(addr_bytes);

        // Check not already deployed
        if self.is_mip20(token_addr)? {
            return Err(MagnusPrecompileError::InvalidInput(
                "token already exists at this address".into(),
            ));
        }

        self.register(token_addr)?;
        Ok(token_addr)
    }
}
