//! Precompile registry -- maps addresses to precompile implementations.

use alloy_primitives::Address;

use crate::addresses;

/// Type alias for a precompile factory function.
pub type PrecompileFactory = Box<
    dyn Fn(Address) -> Option<Box<dyn FnMut(&[u8], Address) -> revm::precompile::PrecompileResult + Send>>
        + Send
        + Sync,
>;

/// Registry of all Magnus native precompiles.
#[derive(Clone, Debug)]
pub struct PrecompileRegistry {
    chain_id: u64,
}

impl PrecompileRegistry {
    pub const fn new(chain_id: u64) -> Self {
        Self { chain_id }
    }

    /// Returns true if the given address is a known precompile.
    pub fn is_precompile(&self, address: &Address) -> bool {
        addresses::is_mip20_prefix(*address)
            || *address == addresses::MIP20_FACTORY
            || *address == addresses::FEE_MANAGER
            || *address == addresses::STABLECOIN_DEX
            || *address == addresses::MIP403_REGISTRY
            || *address == addresses::ORACLE_REGISTRY
            || *address == addresses::ISO20022
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::address;

    #[test]
    fn known_addresses_recognized() {
        let reg = PrecompileRegistry::new(1);
        assert!(reg.is_precompile(&addresses::FEE_MANAGER));
        assert!(reg.is_precompile(&addresses::ORACLE_REGISTRY));
        assert!(reg.is_precompile(&addresses::MIP20_FACTORY));
        assert!(reg.is_precompile(&addresses::STABLECOIN_DEX));
        assert!(reg.is_precompile(&addresses::ISO20022));
    }

    #[test]
    fn unknown_address_not_precompile() {
        let reg = PrecompileRegistry::new(1);
        assert!(!reg.is_precompile(&Address::ZERO));
        assert!(!reg.is_precompile(
            &address!("1111111111111111111111111111111111111111")
        ));
    }

    #[test]
    fn mip20_prefix_recognized() {
        let reg = PrecompileRegistry::new(1);
        // A MIP20 token address: prefix 20C0... + token ID
        let mip20_addr = address!("20C0000000000000000000001234567890abcdef");
        assert!(reg.is_precompile(&mip20_addr));
    }
}
