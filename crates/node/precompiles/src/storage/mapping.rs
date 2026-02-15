//! Mapping<K, V> -- Solidity-compatible storage mapping.

use alloy_primitives::{Address, B256, U256, keccak256};
use std::marker::PhantomData;

/// A storage mapping keyed by K, valued by V.
///
/// Slot calculation: keccak256(abi.encode(key, base_slot))
/// Mirrors Solidity `mapping(K => V)` storage layout.
#[derive(Debug)]
pub struct Mapping<K, V> {
    contract: Address,
    base_slot: U256,
    _phantom: PhantomData<(K, V)>,
}

impl<K, V> Mapping<K, V> {
    /// Create a new mapping for the given contract address and base storage slot.
    pub const fn new(contract: Address, base_slot: U256) -> Self {
        Self {
            contract,
            base_slot,
            _phantom: PhantomData,
        }
    }
}

/// Trait for types that can be used as mapping keys.
pub trait StorageKey {
    /// Encode this key as a 32-byte array for slot computation.
    fn to_slot_bytes(&self) -> [u8; 32];
}

impl StorageKey for Address {
    fn to_slot_bytes(&self) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        bytes[12..32].copy_from_slice(self.as_slice());
        bytes
    }
}

impl StorageKey for U256 {
    fn to_slot_bytes(&self) -> [u8; 32] {
        self.to_be_bytes::<32>()
    }
}

impl StorageKey for B256 {
    fn to_slot_bytes(&self) -> [u8; 32] {
        self.0
    }
}

/// Compute the storage slot for a mapping key.
pub fn mapping_slot(key: &[u8; 32], base_slot: &U256) -> U256 {
    let mut data = [0u8; 64];
    data[..32].copy_from_slice(key);
    data[32..64].copy_from_slice(&base_slot.to_be_bytes::<32>());
    U256::from_be_bytes::<32>(keccak256(data).0)
}

impl<K: StorageKey> Mapping<K, U256> {
    /// Read a `U256` value from the mapping at the given key.
    pub fn read(&self, key: &K) -> U256 {
        let slot = mapping_slot(&key.to_slot_bytes(), &self.base_slot);
        super::sload(self.contract, slot)
    }

    /// Write a `U256` value to the mapping at the given key.
    pub fn write(&mut self, key: &K, value: U256) {
        let slot = mapping_slot(&key.to_slot_bytes(), &self.base_slot);
        super::sstore(self.contract, slot, value);
    }
}

/// Compute the storage slot for a nested mapping: `mapping[key1][key2]`.
///
/// Equivalent to Solidity: `mapping(K1 => mapping(K2 => V))`.
/// slot = keccak256(key2 ++ keccak256(key1 ++ base_slot))
pub fn nested_mapping_slot(
    key1: &[u8; 32],
    key2: &[u8; 32],
    base_slot: &U256,
) -> U256 {
    let inner = mapping_slot(key1, base_slot);
    mapping_slot(key2, &inner)
}

/// Two-level nested mapping: `mapping[K1][K2] -> V`.
#[derive(Debug)]
pub struct NestedMapping<K1, K2, V> {
    contract: Address,
    base_slot: U256,
    _phantom: PhantomData<(K1, K2, V)>,
}

impl<K1: StorageKey, K2: StorageKey, V> NestedMapping<K1, K2, V> {
    pub const fn new(contract: Address, base_slot: U256) -> Self {
        Self { contract, base_slot, _phantom: PhantomData }
    }
}

impl<K1: StorageKey, K2: StorageKey> NestedMapping<K1, K2, U256> {
    pub fn read(&self, key1: &K1, key2: &K2) -> U256 {
        let slot = nested_mapping_slot(
            &key1.to_slot_bytes(),
            &key2.to_slot_bytes(),
            &self.base_slot,
        );
        super::sload(self.contract, slot)
    }

    pub fn write(&mut self, key1: &K1, key2: &K2, value: U256) {
        let slot = nested_mapping_slot(
            &key1.to_slot_bytes(),
            &key2.to_slot_bytes(),
            &self.base_slot,
        );
        super::sstore(self.contract, slot, value);
    }
}

impl<K: StorageKey> Mapping<K, Address> {
    /// Read an `Address` value from the mapping at the given key.
    pub fn read_address(&self, key: &K) -> Address {
        let slot = mapping_slot(&key.to_slot_bytes(), &self.base_slot);
        let val = super::sload(self.contract, slot);
        Address::from_word(B256::from(val.to_be_bytes::<32>()))
    }

    /// Write an `Address` value to the mapping at the given key.
    pub fn write_address(&mut self, key: &K, value: Address) {
        let slot = mapping_slot(&key.to_slot_bytes(), &self.base_slot);
        let val = U256::from_be_bytes::<32>(B256::left_padding_from(value.as_slice()).0);
        super::sstore(self.contract, slot, val);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::address;

    #[test]
    fn mapping_slot_deterministic() {
        let key = Address::ZERO.to_slot_bytes();
        let base = U256::from(0);
        let slot1 = mapping_slot(&key, &base);
        let slot2 = mapping_slot(&key, &base);
        assert_eq!(slot1, slot2);
    }

    #[test]
    fn different_keys_different_slots() {
        let base = U256::from(0);
        let slot1 = mapping_slot(&Address::ZERO.to_slot_bytes(), &base);
        let slot2 = mapping_slot(
            &address!("0000000000000000000000000000000000000001").to_slot_bytes(),
            &base,
        );
        assert_ne!(slot1, slot2);
    }

    #[test]
    fn different_bases_different_slots() {
        let key = Address::ZERO.to_slot_bytes();
        let slot1 = mapping_slot(&key, &U256::from(0));
        let slot2 = mapping_slot(&key, &U256::from(1));
        assert_ne!(slot1, slot2);
    }

    #[test]
    fn nested_mapping_slot_deterministic() {
        let key1 = [1u8; 32];
        let key2 = [2u8; 32];
        let base = U256::from(5);
        let s1 = nested_mapping_slot(&key1, &key2, &base);
        let s2 = nested_mapping_slot(&key1, &key2, &base);
        assert_eq!(s1, s2);
    }

    #[test]
    fn nested_mapping_different_keys_different_slots() {
        let key1 = [1u8; 32];
        let key2a = [2u8; 32];
        let key2b = [3u8; 32];
        let base = U256::from(5);
        let s1 = nested_mapping_slot(&key1, &key2a, &base);
        let s2 = nested_mapping_slot(&key1, &key2b, &base);
        assert_ne!(s1, s2);
    }
}
