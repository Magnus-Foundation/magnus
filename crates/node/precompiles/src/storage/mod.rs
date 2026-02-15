//! Storage abstractions for precompiles.
//!
//! Precompiles access EVM storage slots directly, using keccak256-based
//! slot addressing (same layout as Solidity mappings).

pub mod mapping;
pub use mapping::{Mapping, NestedMapping, StorageKey, nested_mapping_slot};

use alloy_primitives::{Address, U256};

/// Read/write interface to EVM account storage.
///
/// Implementations bridge to the REVM journal for state access.
pub trait StorageBackend {
    /// Load a value from the given storage slot.
    fn sload(&self, address: Address, slot: U256) -> U256;
    /// Store a value into the given storage slot.
    fn sstore(&mut self, address: Address, slot: U256, value: U256);
}

// Thread-local storage backend for precompile execution.
//
// Set by the EVM handler before each precompile call.
// This follows the same pattern as Tempo's StorageCtx.
std::thread_local! {
    static BACKEND: std::cell::RefCell<Option<Box<dyn StorageBackend>>> = const {
        std::cell::RefCell::new(None)
    };
}

/// Execute a closure with the given storage backend.
pub fn with_storage<R>(backend: Box<dyn StorageBackend>, f: impl FnOnce() -> R) -> R {
    BACKEND.with(|cell| {
        *cell.borrow_mut() = Some(backend);
    });
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            BACKEND.with(|cell| {
                *cell.borrow_mut() = None;
            });
        }
    }
    let _guard = Guard;
    f()
}

/// Read a storage slot from the current backend.
pub fn sload(address: Address, slot: U256) -> U256 {
    BACKEND.with(|cell| {
        cell.borrow()
            .as_ref()
            .expect("storage backend not set")
            .sload(address, slot)
    })
}

/// Write a storage slot to the current backend.
pub fn sstore(address: Address, slot: U256, value: U256) {
    BACKEND.with(|cell| {
        cell.borrow_mut()
            .as_mut()
            .expect("storage backend not set")
            .sstore(address, slot, value)
    })
}
