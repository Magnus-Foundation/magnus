//! Router selector registry for fee-token inference (spec §11.3).
//!
//! Governance registers `(router, selector) → arg_index` triples. When a
//! transaction targets a registered router, the protocol decodes the token
//! address at `arg_index` and uses it as the fee token.

use alloy::primitives::{B256, FixedBytes};
use magnus_precompiles_macros::Storable;

#[derive(Clone, Debug, Default, PartialEq, Eq, Storable)]
pub struct RouterDescriptor {
    pub registered: bool,
    pub token_input_arg_index: u8,
}

/// Pads a 4-byte selector into a B256 storage key (left-aligned, rest zero).
pub fn selector_key(selector: FixedBytes<4>) -> B256 {
    let mut key = [0u8; 32];
    key[..4].copy_from_slice(selector.as_slice());
    B256::from(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_key_left_aligns_4_bytes_with_zero_padding() {
        let sel = FixedBytes::<4>::from([0xde, 0xad, 0xbe, 0xef]);
        let key = selector_key(sel);
        assert_eq!(&key.as_slice()[..4], sel.as_slice());
        assert!(key.as_slice()[4..].iter().all(|b| *b == 0));
    }

    #[test]
    fn selector_key_distinguishes_different_selectors() {
        let a = FixedBytes::<4>::from([0xaa, 0xaa, 0xaa, 0xaa]);
        let b = FixedBytes::<4>::from([0xbb, 0xbb, 0xbb, 0xbb]);
        assert_ne!(selector_key(a), selector_key(b));
    }

    #[test]
    fn router_descriptor_default_is_unregistered() {
        let d = RouterDescriptor::default();
        assert!(!d.registered);
        assert_eq!(d.token_input_arg_index, 0);
    }
}
