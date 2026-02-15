//! Magnus RPC types.

mod header;
pub use header::MagnusHeaderResponse;

mod request;
pub use request::{FeeToken, MagnusCallBuilderExt, MagnusTransactionRequest};

mod receipt;
pub use receipt::MagnusTransactionReceipt;

#[cfg(feature = "magnus-compat")]
mod compat;

/// Various helper types for paginated queries.
pub mod pagination;
