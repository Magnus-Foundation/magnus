//! Core trait abstractions for Magnus storage and consensus.

#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/refcell/magnus/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod error;
pub use error::StateDbError;

mod state;
pub use state::{StateDb, StateDbRead, StateDbWrite};
