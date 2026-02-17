//! SQLite-based local audit store for payment tracking.

use eyre::Result;
use rusqlite::{Connection, params};
use std::sync::Mutex;

/// Payment status in the audit trail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PaymentStatus {
    /// Payment initiation received.
    Received,
    /// On-chain transaction submitted.
    Submitted,
    /// On-chain transaction confirmed.
    Confirmed,
    /// ISO 20022 message forwarded to bank.
    Forwarded,
    /// Payment failed.
    Failed,
}

impl PaymentStatus {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Received => "received",
            Self::Submitted => "submitted",
            Self::Confirmed => "confirmed",
            Self::Forwarded => "forwarded",
            Self::Failed => "failed",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "received" => Self::Received,
            "submitted" => Self::Submitted,
            "confirmed" => Self::Confirmed,
            "forwarded" => Self::Forwarded,
            "failed" => Self::Failed,
            _ => Self::Received,
        }
    }
}

/// A payment record in the audit store.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PaymentRecord {
    /// Auto-increment ID.
    pub id: i64,
    /// ISO 20022 end-to-end identification.
    pub end_to_end_id: String,
    /// Current status.
    pub status: PaymentStatus,
    /// On-chain transaction hash (if submitted).
    pub chain_tx_hash: Option<String>,
    /// IPFS hash of the archived ISO message.
    pub ipfs_hash: Option<String>,
    /// Account (debtor or creditor address).
    pub account: String,
    /// Creation timestamp.
    pub created_at: String,
    /// Last forwarded timestamp.
    pub forwarded_at: Option<String>,
}

/// SQLite audit store (thread-safe via internal Mutex).
pub struct AuditStore {
    conn: Mutex<Connection>,
}

impl AuditStore {
    /// Open (or create) the audit database.
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS payments (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                end_to_end_id TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'received',
                chain_tx_hash TEXT,
                ipfs_hash TEXT,
                account TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                forwarded_at TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_payments_e2e ON payments(end_to_end_id);
            CREATE INDEX IF NOT EXISTS idx_payments_account ON payments(account);",
        )?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    /// Open an in-memory database (for testing).
    pub fn open_memory() -> Result<Self> {
        Self::open(":memory:")
    }

    /// Insert a new payment record.
    pub fn insert_payment(
        &self,
        end_to_end_id: &str,
        account: &str,
    ) -> Result<i64> {
        let conn = self.conn.lock().map_err(|e| eyre::eyre!("lock poisoned: {e}"))?;
        conn.execute(
            "INSERT INTO payments (end_to_end_id, account) VALUES (?1, ?2)",
            params![end_to_end_id, account],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Update a payment's status.
    pub fn update_status(
        &self,
        end_to_end_id: &str,
        status: PaymentStatus,
    ) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| eyre::eyre!("lock poisoned: {e}"))?;
        conn.execute(
            "UPDATE payments SET status = ?1 WHERE end_to_end_id = ?2",
            params![status.as_str(), end_to_end_id],
        )?;
        Ok(())
    }

    /// Update chain tx hash after on-chain submission.
    pub fn update_chain_tx(
        &self,
        end_to_end_id: &str,
        chain_tx_hash: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| eyre::eyre!("lock poisoned: {e}"))?;
        conn.execute(
            "UPDATE payments SET chain_tx_hash = ?1, status = 'submitted' WHERE end_to_end_id = ?2",
            params![chain_tx_hash, end_to_end_id],
        )?;
        Ok(())
    }

    /// Update IPFS hash after archival.
    pub fn update_ipfs_hash(
        &self,
        end_to_end_id: &str,
        ipfs_hash: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| eyre::eyre!("lock poisoned: {e}"))?;
        conn.execute(
            "UPDATE payments SET ipfs_hash = ?1 WHERE end_to_end_id = ?2",
            params![ipfs_hash, end_to_end_id],
        )?;
        Ok(())
    }

    /// Query a payment by end-to-end ID.
    pub fn query_by_end_to_end_id(&self, end_to_end_id: &str) -> Result<Option<PaymentRecord>> {
        let conn = self.conn.lock().map_err(|e| eyre::eyre!("lock poisoned: {e}"))?;
        let mut stmt = conn.prepare(
            "SELECT id, end_to_end_id, status, chain_tx_hash, ipfs_hash, account, created_at, forwarded_at
             FROM payments WHERE end_to_end_id = ?1 LIMIT 1",
        )?;

        let record = stmt.query_row(params![end_to_end_id], |row| {
            Ok(PaymentRecord {
                id: row.get(0)?,
                end_to_end_id: row.get(1)?,
                status: PaymentStatus::from_str(&row.get::<_, String>(2)?),
                chain_tx_hash: row.get(3)?,
                ipfs_hash: row.get(4)?,
                account: row.get(5)?,
                created_at: row.get(6)?,
                forwarded_at: row.get(7)?,
            })
        });

        match record {
            Ok(r) => Ok(Some(r)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Query all payments for a given account.
    pub fn query_by_account(&self, account: &str) -> Result<Vec<PaymentRecord>> {
        let conn = self.conn.lock().map_err(|e| eyre::eyre!("lock poisoned: {e}"))?;
        let mut stmt = conn.prepare(
            "SELECT id, end_to_end_id, status, chain_tx_hash, ipfs_hash, account, created_at, forwarded_at
             FROM payments WHERE account = ?1 ORDER BY created_at DESC",
        )?;

        let records = stmt
            .query_map(params![account], |row| {
                Ok(PaymentRecord {
                    id: row.get(0)?,
                    end_to_end_id: row.get(1)?,
                    status: PaymentStatus::from_str(&row.get::<_, String>(2)?),
                    chain_tx_hash: row.get(3)?,
                    ipfs_hash: row.get(4)?,
                    account: row.get(5)?,
                    created_at: row.get(6)?,
                    forwarded_at: row.get(7)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_query() {
        let store = AuditStore::open_memory().unwrap();

        let id = store.insert_payment("E2E-001", "0xAlice").unwrap();
        assert_eq!(id, 1);

        let record = store.query_by_end_to_end_id("E2E-001").unwrap();
        assert!(record.is_some());
        let record = record.unwrap();
        assert_eq!(record.end_to_end_id, "E2E-001");
        assert_eq!(record.account, "0xAlice");
        assert_eq!(record.status, PaymentStatus::Received);
        assert!(record.chain_tx_hash.is_none());
    }

    #[test]
    fn test_update_status_flow() {
        let store = AuditStore::open_memory().unwrap();

        store.insert_payment("E2E-002", "0xBob").unwrap();

        store.update_chain_tx("E2E-002", "0xdeadbeef").unwrap();
        let record = store.query_by_end_to_end_id("E2E-002").unwrap().unwrap();
        assert_eq!(record.status, PaymentStatus::Submitted);
        assert_eq!(record.chain_tx_hash.as_deref(), Some("0xdeadbeef"));

        store.update_status("E2E-002", PaymentStatus::Confirmed).unwrap();
        let record = store.query_by_end_to_end_id("E2E-002").unwrap().unwrap();
        assert_eq!(record.status, PaymentStatus::Confirmed);

        store.update_ipfs_hash("E2E-002", "QmTestHash123").unwrap();
        let record = store.query_by_end_to_end_id("E2E-002").unwrap().unwrap();
        assert_eq!(record.ipfs_hash.as_deref(), Some("QmTestHash123"));
    }

    #[test]
    fn test_query_by_account() {
        let store = AuditStore::open_memory().unwrap();

        store.insert_payment("E2E-010", "0xAlice").unwrap();
        store.insert_payment("E2E-011", "0xAlice").unwrap();
        store.insert_payment("E2E-012", "0xBob").unwrap();

        let alice_records = store.query_by_account("0xAlice").unwrap();
        assert_eq!(alice_records.len(), 2);

        let bob_records = store.query_by_account("0xBob").unwrap();
        assert_eq!(bob_records.len(), 1);

        let unknown = store.query_by_account("0xUnknown").unwrap();
        assert!(unknown.is_empty());
    }

    #[test]
    fn test_query_nonexistent() {
        let store = AuditStore::open_memory().unwrap();
        let record = store.query_by_end_to_end_id("DOES-NOT-EXIST").unwrap();
        assert!(record.is_none());
    }
}
