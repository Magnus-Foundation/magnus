//! [`Digest`] is a wrapper around [`B256`] to use eth block hash in Magnus simplex.

use std::ops::Deref;

use alloy_primitives::B256;
use magnus_codec::{FixedSize, Read, ReadExt as _, Write};
use magnus_utils::{Array, Span};

/// Wrapper around [`B256`] to use it in places requiring [`magnus_cryptography::Digest`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[repr(transparent)]
pub struct Digest(pub B256);

impl Array for Digest {}

impl AsRef<[u8]> for Digest {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl Deref for Digest {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl magnus_math::algebra::Random for Digest {
    /// Generate a random digest.
    ///
    /// # Note
    ///
    /// One-to-one copy of [`magnus_cryptography::Digest`]
    /// for [`magnus_cryptography::sha256::Digest`].
    fn random(mut rng: impl rand_core::CryptoRngCore) -> Self {
        let mut array = B256::ZERO;
        rng.fill_bytes(&mut *array);
        Self(array)
    }
}

impl magnus_cryptography::Digest for Digest {
    const EMPTY: Self = Self(B256::ZERO);
}

impl FixedSize for Digest {
    const SIZE: usize = 32;
}

impl std::fmt::Display for Digest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Read for Digest {
    type Cfg = ();

    fn read_cfg(
        buf: &mut impl bytes::Buf,
        _cfg: &Self::Cfg,
    ) -> Result<Self, magnus_codec::Error> {
        let array = <[u8; 32]>::read(buf)?;
        Ok(Self(B256::new(array)))
    }
}

impl Span for Digest {}

impl Write for Digest {
    fn write(&self, buf: &mut impl bytes::BufMut) {
        self.0.write(buf)
    }
}
