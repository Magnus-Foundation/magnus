//! Mainly aliases to define consensus within magnus.

pub(crate) mod application;
pub(crate) mod block;
pub(crate) mod digest;
pub(crate) mod engine;

pub use digest::Digest;

pub use engine::{Builder, Engine};
