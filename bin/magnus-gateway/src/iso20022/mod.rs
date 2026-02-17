//! ISO 20022 message builder, parser, and validator.
//!
//! Supports the following message types:
//! - **pain.001** — Customer Credit Transfer Initiation (inbound from bank)
//! - **pacs.008** — FI-to-FI Customer Credit Transfer (outbound after on-chain settlement)
//! - **camt.053** — Bank-to-Customer Statement (account statement generation)
//! - **camt.054** — Bank-to-Customer Debit/Credit Notification (real-time push)

pub mod builder;
pub mod parser;
pub mod types;
