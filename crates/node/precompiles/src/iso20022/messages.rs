//! ISO 20022 message structures.

use alloy_primitives::{Address, U256};

/// pacs.008 Credit Transfer Instruction.
///
/// Simplified on-chain representation of the ISO 20022 pacs.008 message.
#[derive(Debug, Clone)]
pub struct CreditTransfer {
    /// Unique message identifier
    pub msg_id: [u8; 32],
    /// Debtor (sender) address
    pub debtor: Address,
    /// Creditor (receiver) address
    pub creditor: Address,
    /// Amount in smallest unit
    pub amount: U256,
    /// Currency code (ISO 4217, e.g., "VND", "USD")
    pub currency: [u8; 3],
    /// Settlement date (unix timestamp)
    pub settlement_date: u64,
    /// Remittance information (purpose code)
    pub remittance_info: [u8; 4],
}

/// pacs.002 Payment Status Report.
#[derive(Debug, Clone)]
pub struct PaymentStatusReport {
    /// Original message ID this status refers to
    pub original_msg_id: [u8; 32],
    /// Status code
    pub status: PaymentStatus,
    /// Transaction hash (on-chain reference)
    pub tx_hash: [u8; 32],
}

/// Payment status codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PaymentStatus {
    Accepted = 0,
    Pending = 1,
    Rejected = 2,
    Settled = 3,
}
