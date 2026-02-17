//! Shared ISO 20022 data types.

use serde::{Deserialize, Serialize};

/// Core payment instruction extracted from pain.001.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentInstruction {
    /// Message identification.
    pub msg_id: String,
    /// Creation date-time (ISO 8601).
    pub creation_date_time: String,
    /// Number of transactions in the batch.
    pub nb_of_txs: u32,
    /// Control sum (total amount).
    pub ctrl_sum: Option<String>,
    /// Individual payment transactions.
    pub transactions: Vec<CreditTransfer>,
}

/// Individual credit transfer within a payment instruction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditTransfer {
    /// End-to-end identification (unique per payment).
    pub end_to_end_id: String,
    /// Amount with currency.
    pub amount: Amount,
    /// Debtor (payer) information.
    pub debtor: Party,
    /// Debtor's account.
    pub debtor_account: Account,
    /// Creditor (payee) information.
    pub creditor: Party,
    /// Creditor's account.
    pub creditor_account: Account,
    /// Purpose code (e.g. SALA, SUPP, TAXS).
    pub purpose_code: Option<String>,
    /// Unstructured remittance information.
    pub remittance_info: Option<String>,
}

/// Monetary amount with currency code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Amount {
    /// ISO 4217 currency code (e.g. "USD", "VND").
    pub currency: String,
    /// Amount as decimal string (e.g. "1000.00").
    pub value: String,
}

/// Party identification (debtor or creditor).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Party {
    /// Party name.
    pub name: String,
    /// BIC/SWIFT code (optional).
    pub bic: Option<String>,
}

/// Account identification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    /// IBAN or on-chain address.
    pub id: String,
}

/// Settlement confirmation data for pacs.008 generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementData {
    /// Original payment instruction.
    pub instruction: CreditTransfer,
    /// On-chain transaction hash.
    pub chain_tx_hash: String,
    /// Settlement timestamp (ISO 8601).
    pub settlement_time: String,
    /// Instructing agent BIC.
    pub instructing_agent: String,
    /// Instructed agent BIC.
    pub instructed_agent: String,
}

/// Account statement entry for camt.053.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatementEntry {
    /// Entry reference.
    pub entry_ref: String,
    /// Amount with currency.
    pub amount: Amount,
    /// Credit/Debit indicator: "CRDT" or "DBIT".
    pub cd_indicator: String,
    /// Booking date (ISO 8601).
    pub booking_date: String,
    /// Value date (ISO 8601).
    pub value_date: String,
    /// End-to-end ID from the original payment.
    pub end_to_end_id: String,
    /// Remittance information.
    pub remittance_info: Option<String>,
}

/// Notification entry for camt.054.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationEntry {
    /// Notification ID.
    pub ntfctn_id: String,
    /// Account holder.
    pub account_id: String,
    /// The statement entry being notified.
    pub entry: StatementEntry,
}
