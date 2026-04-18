use alloy_primitives::Address;
use serde::{Deserialize, Serialize};
use magnus_primitives::{MagnusTxEnvelope, MagnusTxType};

pub type Transaction = alloy_rpc_types_eth::Transaction<MagnusTxEnvelope>;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionsResponse {
    /// Cursor for next page, null if no more results
    pub next_cursor: Option<String>,
    /// Array of items matching the input query
    pub transactions: Vec<Transaction>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionsFilter {
    /// Filter by sender address (from)
    from: Option<Address>,
    /// Filter by recipient address (to)
    to: Option<Address>,
    /// Transaction type
    #[serde(rename = "type")]
    type_: Option<MagnusTxType>,
}
