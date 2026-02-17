//! MIP20-specific indexing: tokens, role changes, balances, and paginated queries.

use alloy_primitives::{Address, BlockNumber, B256, Log, TxHash, U256};
use alloy_sol_types::SolEvent;
use parking_lot::RwLock;
use std::collections::BTreeMap;
use tracing::debug;

use crate::events;

/// Indexed MIP20 token metadata.
#[derive(Clone, Debug)]
pub struct IndexedToken {
    /// Token contract address.
    pub address: Address,
    /// Block timestamp when token was created.
    pub created_at: u64,
    /// Creator (admin) address.
    pub creator: Address,
    /// Currency code (e.g., "USD").
    pub currency: String,
    /// Token decimals.
    pub decimals: u32,
    /// Token name.
    pub name: String,
    /// Whether the token is paused.
    pub paused: bool,
    /// Quote token address.
    pub quote_token: Address,
    /// Maximum supply cap.
    pub supply_cap: u128,
    /// Token symbol.
    pub symbol: String,
    /// Factory-assigned token ID.
    pub token_id: u64,
    /// Current total supply.
    pub total_supply: u128,
    /// Current transfer policy ID.
    pub transfer_policy_id: u64,
}

/// Indexed role change event.
#[derive(Clone, Debug)]
pub struct IndexedRoleChange {
    /// Account that received/lost the role.
    pub account: Address,
    /// Block number where change occurred.
    pub block_number: BlockNumber,
    /// Whether role was granted (`true`) or revoked (`false`).
    pub granted: bool,
    /// Role identifier.
    pub role: B256,
    /// Address that made the change.
    pub sender: Address,
    /// Block timestamp.
    pub timestamp: u64,
    /// Token contract address that emitted the event.
    pub token: Address,
    /// Transaction hash.
    pub transaction_hash: TxHash,
}

/// In-memory store for MIP20 token data, role changes, and balances.
#[derive(Debug, Default)]
pub struct TokenStore {
    /// All known MIP20 tokens, keyed by address.
    tokens: RwLock<BTreeMap<Address, IndexedToken>>,
    /// Role changes ordered by insertion (block_number asc).
    role_changes: RwLock<Vec<IndexedRoleChange>>,
    /// Token balances: token_address -> account -> balance.
    balances: RwLock<BTreeMap<Address, BTreeMap<Address, U256>>>,
    /// Token roles: token_address -> account -> Vec<role>.
    account_roles: RwLock<BTreeMap<Address, BTreeMap<Address, Vec<B256>>>>,
    /// Head block number that has been indexed.
    head: RwLock<BlockNumber>,
}

impl TokenStore {
    /// Creates a new empty token store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the current head block number.
    #[must_use]
    pub fn head_block(&self) -> BlockNumber {
        *self.head.read()
    }

    /// Inserts or updates a token in the store.
    pub fn upsert_token(&self, token: IndexedToken) {
        self.tokens.write().insert(token.address, token);
    }

    /// Returns all indexed tokens.
    pub fn get_all_tokens(&self) -> Vec<IndexedToken> {
        self.tokens.read().values().cloned().collect()
    }

    /// Paginated token query with optional filters.
    pub fn get_tokens(
        &self,
        currency: Option<&str>,
        creator: Option<Address>,
        paused: Option<bool>,
        name: Option<&str>,
        symbol: Option<&str>,
        cursor: Option<&str>,
        limit: usize,
    ) -> (Vec<IndexedToken>, Option<String>) {
        let tokens = self.tokens.read();
        let mut results: Vec<_> = tokens
            .values()
            .filter(|t| {
                if let Some(c) = currency {
                    if !t.currency.eq_ignore_ascii_case(c) {
                        return false;
                    }
                }
                if let Some(c) = creator {
                    if t.creator != c {
                        return false;
                    }
                }
                if let Some(p) = paused {
                    if t.paused != p {
                        return false;
                    }
                }
                if let Some(n) = name {
                    if !t.name.to_lowercase().contains(&n.to_lowercase()) {
                        return false;
                    }
                }
                if let Some(s) = symbol {
                    if !t.symbol.eq_ignore_ascii_case(s) {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        // Sort by token_id ascending (stable order).
        results.sort_by_key(|t| t.token_id);

        // Apply cursor (token_id as string).
        if let Some(cursor_str) = cursor {
            if let Ok(cursor_id) = cursor_str.parse::<u64>() {
                results.retain(|t| t.token_id > cursor_id);
            }
        }

        let next_cursor = if results.len() > limit {
            results.truncate(limit);
            results.last().map(|t| t.token_id.to_string())
        } else {
            None
        };

        (results, next_cursor)
    }

    /// Paginated tokens-by-address query: returns tokens where the account has a balance or roles.
    pub fn get_tokens_by_address(
        &self,
        address: Address,
        currency: Option<&str>,
        cursor: Option<&str>,
        limit: usize,
    ) -> (Vec<(IndexedToken, U256, Vec<B256>)>, Option<String>) {
        let tokens = self.tokens.read();
        let balances = self.balances.read();
        let roles = self.account_roles.read();

        let mut results: Vec<(IndexedToken, U256, Vec<B256>)> = tokens
            .values()
            .filter_map(|token| {
                let balance = balances
                    .get(&token.address)
                    .and_then(|m| m.get(&address))
                    .copied()
                    .unwrap_or(U256::ZERO);
                let account_roles = roles
                    .get(&token.address)
                    .and_then(|m| m.get(&address))
                    .cloned()
                    .unwrap_or_default();

                // Only include if the account has a balance or roles.
                if balance.is_zero() && account_roles.is_empty() {
                    return None;
                }

                if let Some(c) = currency {
                    if !token.currency.eq_ignore_ascii_case(c) {
                        return None;
                    }
                }

                Some((token.clone(), balance, account_roles))
            })
            .collect();

        results.sort_by_key(|(t, _, _)| t.token_id);

        if let Some(cursor_str) = cursor {
            if let Ok(cursor_id) = cursor_str.parse::<u64>() {
                results.retain(|(t, _, _)| t.token_id > cursor_id);
            }
        }

        let next_cursor = if results.len() > limit {
            results.truncate(limit);
            results.last().map(|(t, _, _)| t.token_id.to_string())
        } else {
            None
        };

        (results, next_cursor)
    }

    /// Paginated role history query.
    pub fn get_role_history(
        &self,
        account: Option<Address>,
        token: Option<Address>,
        role: Option<B256>,
        granted: Option<bool>,
        sender: Option<Address>,
        cursor: Option<&str>,
        limit: usize,
    ) -> (Vec<IndexedRoleChange>, Option<String>) {
        let changes = self.role_changes.read();
        let mut results: Vec<_> = changes
            .iter()
            .filter(|c| {
                if let Some(a) = account {
                    if c.account != a {
                        return false;
                    }
                }
                if let Some(t) = token {
                    if c.token != t {
                        return false;
                    }
                }
                if let Some(r) = role {
                    if c.role != r {
                        return false;
                    }
                }
                if let Some(g) = granted {
                    if c.granted != g {
                        return false;
                    }
                }
                if let Some(s) = sender {
                    if c.sender != s {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        // Sort by block_number descending (most recent first).
        results.sort_by(|a, b| b.block_number.cmp(&a.block_number));

        // Cursor is "block_number:tx_hash" for stable pagination.
        if let Some(cursor_str) = cursor {
            if let Some((bn_str, _hash_str)) = cursor_str.split_once(':') {
                if let Ok(bn) = bn_str.parse::<u64>() {
                    results.retain(|c| c.block_number < bn);
                }
            }
        }

        let next_cursor = if results.len() > limit {
            results.truncate(limit);
            results
                .last()
                .map(|c| format!("{}:{}", c.block_number, c.transaction_hash))
        } else {
            None
        };

        (results, next_cursor)
    }

    /// Process a block's logs and index MIP20 events.
    ///
    /// Each entry in `logs` is a `(Log, TxHash)` pair mapping each log to its transaction.
    pub fn index_block(
        &self,
        block_number: BlockNumber,
        timestamp: u64,
        logs: &[(Log, TxHash)],
    ) {
        let mut token_count = 0u32;
        let mut role_count = 0u32;
        let mut transfer_count = 0u32;

        for (log, tx_hash) in logs {
            // Try to decode TokenCreated from MIP20Factory.
            if let Ok(created) = events::TokenCreated::decode_log(log) {
                debug!(
                    token = %created.token,
                    name = %created.name,
                    "indexed TokenCreated"
                );
                self.upsert_token(IndexedToken {
                    address: created.token,
                    created_at: timestamp,
                    creator: created.admin,
                    currency: created.currency.clone(),
                    decimals: 6, // MIP20 default
                    name: created.name.clone(),
                    paused: false,
                    quote_token: created.quoteToken,
                    supply_cap: 0,
                    symbol: created.symbol.clone(),
                    token_id: token_count.into(),
                    total_supply: 0,
                    transfer_policy_id: 0,
                });
                token_count += 1;
                continue;
            }

            // Try to decode RoleMembershipUpdated.
            if let Ok(role_event) = events::RoleMembershipUpdated::decode_log(log) {
                let token_addr = log.address;
                let change = IndexedRoleChange {
                    account: role_event.account,
                    block_number,
                    granted: role_event.hasRole,
                    role: role_event.role,
                    sender: role_event.sender,
                    timestamp,
                    token: token_addr,
                    transaction_hash: *tx_hash,
                };
                self.role_changes.write().push(change);

                // Update account_roles index.
                let mut roles = self.account_roles.write();
                let token_roles = roles.entry(token_addr).or_default();
                let account_roles = token_roles.entry(role_event.account).or_default();
                if role_event.hasRole {
                    if !account_roles.contains(&role_event.role) {
                        account_roles.push(role_event.role);
                    }
                } else {
                    account_roles.retain(|r| *r != role_event.role);
                }
                role_count += 1;
                continue;
            }

            // Try to decode Transfer.
            if let Ok(transfer) = events::Transfer::decode_log(log) {
                let token_addr = log.address;
                let mut balances = self.balances.write();
                let token_balances = balances.entry(token_addr).or_default();

                // Debit sender (skip zero address for mints).
                if !transfer.from.is_zero() {
                    let from_bal = token_balances.entry(transfer.from).or_insert(U256::ZERO);
                    *from_bal = from_bal.saturating_sub(transfer.amount);
                }
                // Credit receiver (skip zero address for burns).
                if !transfer.to.is_zero() {
                    let to_bal = token_balances.entry(transfer.to).or_insert(U256::ZERO);
                    *to_bal = to_bal.saturating_add(transfer.amount);
                }
                transfer_count += 1;
                continue;
            }
        }

        *self.head.write() = block_number;

        if token_count > 0 || role_count > 0 || transfer_count > 0 {
            debug!(
                block_number,
                token_count,
                role_count,
                transfer_count,
                "indexed MIP20 events"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{address, b256, Bytes};

    use super::*;

    fn make_test_token(id: u64, currency: &str) -> IndexedToken {
        IndexedToken {
            address: Address::with_last_byte(id as u8),
            created_at: 1000 + id,
            creator: Address::ZERO,
            currency: currency.to_string(),
            decimals: 6,
            name: format!("Token {id}"),
            paused: false,
            quote_token: Address::ZERO,
            supply_cap: 1_000_000,
            symbol: format!("TK{id}"),
            token_id: id,
            total_supply: 0,
            transfer_policy_id: 0,
        }
    }

    #[test]
    fn get_tokens_basic_pagination() {
        let store = TokenStore::new();
        for i in 0..25 {
            store.upsert_token(make_test_token(i, "USD"));
        }

        let (page1, cursor1) = store.get_tokens(None, None, None, None, None, None, 10);
        assert_eq!(page1.len(), 10);
        assert!(cursor1.is_some());

        let (page2, cursor2) =
            store.get_tokens(None, None, None, None, None, cursor1.as_deref(), 10);
        assert_eq!(page2.len(), 10);
        assert!(cursor2.is_some());

        let (page3, cursor3) =
            store.get_tokens(None, None, None, None, None, cursor2.as_deref(), 10);
        assert_eq!(page3.len(), 5);
        assert!(cursor3.is_none());
    }

    #[test]
    fn get_tokens_currency_filter() {
        let store = TokenStore::new();
        store.upsert_token(make_test_token(1, "USD"));
        store.upsert_token(make_test_token(2, "EUR"));
        store.upsert_token(make_test_token(3, "USD"));

        let (results, _) = store.get_tokens(Some("USD"), None, None, None, None, None, 100);
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|t| t.currency == "USD"));
    }

    #[test]
    fn get_tokens_by_address_with_balance() {
        let store = TokenStore::new();
        let account = address!("0x1111111111111111111111111111111111111111");
        let token_addr = Address::with_last_byte(1);

        store.upsert_token(IndexedToken {
            address: token_addr,
            ..make_test_token(1, "USD")
        });

        // Give the account a balance.
        store
            .balances
            .write()
            .entry(token_addr)
            .or_default()
            .insert(account, U256::from(1000));

        let (results, _) = store.get_tokens_by_address(account, None, None, 100);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, U256::from(1000));
    }

    #[test]
    fn get_role_history_filters() {
        let store = TokenStore::new();
        let token = address!("0x2222222222222222222222222222222222222222");
        let account = address!("0x3333333333333333333333333333333333333333");
        let role = b256!("0x0000000000000000000000000000000000000000000000000000000000000001");

        store.role_changes.write().push(IndexedRoleChange {
            account,
            block_number: 100,
            granted: true,
            role,
            sender: Address::ZERO,
            timestamp: 1000,
            token,
            transaction_hash: B256::ZERO,
        });
        store.role_changes.write().push(IndexedRoleChange {
            account,
            block_number: 200,
            granted: false,
            role,
            sender: Address::ZERO,
            timestamp: 2000,
            token,
            transaction_hash: B256::repeat_byte(1),
        });

        // Filter by granted=true.
        let (results, _) =
            store.get_role_history(Some(account), Some(token), None, Some(true), None, None, 100);
        assert_eq!(results.len(), 1);
        assert!(results[0].granted);
    }

    #[test]
    fn index_block_transfer_updates_balances() {
        use alloy_sol_types::SolEvent;

        let store = TokenStore::new();
        let token_addr = address!("0x20c0000000000000000000000000000000000001");
        let from = address!("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let to = address!("0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");

        // Seed initial balance.
        store
            .balances
            .write()
            .entry(token_addr)
            .or_default()
            .insert(from, U256::from(1000));

        // Build a Transfer log.
        let transfer = events::Transfer {
            from,
            to,
            amount: U256::from(300),
        };
        let topics: Vec<B256> = transfer
            .encode_topics()
            .into_iter()
            .map(|t| B256::from(t.0))
            .collect();
        let data: Bytes = transfer.encode_data().into();
        let transfer_log =
            Log::new(token_addr, topics, data).expect("valid log");
        let tx_hash = B256::repeat_byte(0x42);

        store.index_block(1, 1000, &[(transfer_log, tx_hash)]);

        let balances = store.balances.read();
        let token_bals = balances.get(&token_addr).unwrap();
        assert_eq!(*token_bals.get(&from).unwrap(), U256::from(700));
        assert_eq!(*token_bals.get(&to).unwrap(), U256::from(300));
    }
}
