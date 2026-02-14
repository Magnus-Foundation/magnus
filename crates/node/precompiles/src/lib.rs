//! Magnus native precompiles for multi-currency stablecoin gas fees and oracle.
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

pub mod error;
pub mod storage;
pub mod registry;

// Token system
pub mod mip20;
pub mod mip20_factory;

// Fee system
pub mod fee_manager;
pub mod fee_amm;

// Oracle system
pub mod oracle_registry;

// Compliance
pub mod mip403_registry;

// ISO 20022
pub mod iso20022;

/// Fixed precompile addresses.
pub mod addresses {
    use alloy_primitives::{address, Address};

    /// MIP20 prefix: tokens live at 0x20C0...{token_id}
    pub const MIP20_PREFIX: [u8; 12] = hex_literal::hex!("20C000000000000000000000");

    /// MIP20 Factory: deploys new MIP20 tokens
    pub const MIP20_FACTORY: Address = address!("20FC20FC20FC20FC20FC20FC20FC20FC20FC20FC");

    /// Fee Manager: collects/refunds/swaps gas fees
    pub const FEE_MANAGER: Address = address!("feecfeecfeecfeecfeecfeecfeecfeecfeecfeec");

    /// Stablecoin DEX: AMM for fee token swaps
    pub const STABLECOIN_DEX: Address = address!("dec0dec0dec0dec0dec0dec0dec0dec0dec0dec0");

    /// MIP403 Compliance Registry
    pub const MIP403_REGISTRY: Address = address!("403C403C403C403C403C403C403C403C403C403C");

    /// Oracle Registry: SortedOracles pattern
    pub const ORACLE_REGISTRY: Address = address!("02AC1E02AC1E02AC1E02AC1E02AC1E02AC1E02AC");

    /// ISO 20022 Message Processor
    pub const ISO20022: Address = address!("1502200215022002150220021502200215022002");

    /// Check if address has MIP20 prefix
    pub fn is_mip20_prefix(addr: Address) -> bool {
        addr.as_slice()[..12] == MIP20_PREFIX
    }
}

use alloy_primitives::Address;

/// Trait for precompile implementations.
pub trait Precompile {
    /// Execute the precompile with the given calldata and sender.
    fn call(
        &mut self,
        calldata: &[u8],
        msg_sender: Address,
    ) -> revm::precompile::PrecompileResult;
}
