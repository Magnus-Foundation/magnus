//! Node builder for constructing Magnus nodes with consensus components.
#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/refcell/magnus/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![feature(associated_type_defaults)]

use magnus_indexer as _;

mod builder;
pub use builder::NodeBuilder;

mod traits;
pub use traits::{ConsensusProvider, NodeComponents, Random};
