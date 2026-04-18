//! Tempo transaction pool implementation.

#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod transaction;
pub mod validator;

pub use transaction::{KeychainSubject, RevokedKeys, SpendingLimitUpdates};

// Tempo pool module with 2D nonce support
pub mod magnus_pool;

// The main Tempo transaction pool type that handles both protocol and 2D nonces
pub use magnus_pool::MagnusTransactionPool;

pub mod amm;
pub mod best;
pub mod maintain;
pub mod metrics;
pub mod paused;
pub mod tt_2d_pool;

pub use maintain::MagnusPoolUpdates;

pub use metrics::{AA2dPoolMetrics, MagnusPoolMaintenanceMetrics};
pub use tt_2d_pool::{AA2dPool, AA2dPoolConfig, AASequenceId, DEFAULT_MAX_TXS_PER_SENDER};

#[cfg(test)]
pub(crate) mod test_utils;
