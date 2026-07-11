use anyhow::{Context, Result, bail};
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};

/// Install the reimbursement settlement tables. This is idempotent and is
/// intended to be called from the main schema migration.
pub fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS reimbursement_payments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            reimburser_id INTEGER NOT NULL REFERENCES reimbursers(id) ON DELETE RESTRICT,
            amount INTEGER NOT NULL CHECK (amount > 0),
            received_at INTEGER NOT NULL,
            source_transaction_id TEXT UNIQUE REFERENCES transactions(id) ON DELETE RESTRICT,
            reference TEXT,
            note TEXT,
            created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS reimbursement_allocations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            payment_id INTEGER NOT NULL REFERENCES reimbursement_payments(id) ON DELETE CASCADE,
            transaction_id TEXT NOT NULL REFERENCES transactions(id) ON DELETE RESTRICT,
            amount INTEGER NOT NULL CHECK (amount > 0),
            created_at INTEGER NOT NULL,
            UNIQUE(payment_id, transaction_id)
        );
        CREATE INDEX IF NOT EXISTS idx_reimbursement_payments_reimburser
            ON reimbursement_payments(reimburser_id, received_at);
        CREATE INDEX IF NOT EXISTS idx_reimbursement_allocations_transaction
            ON reimbursement_allocations(transaction_id);",
    )?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReimbursementPayment {
    pub id: i64,
    pub reimburser_id: i64,
    pub amount: i64,
    pub received_at: i64,
    pub source_transaction_id: Option<String>,
    pub reference: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocationRequest<'a> {
    pub transaction_id: &'a str,
    pub amount: i64,
}

impl ReimbursementPayment {
    /// Record a payment and atomically distribute it across reimbursable expenses.
    /// Amounts are positive cents. Unallocated payment balances are allowed.
    pub fn create(
        conn: &Connection,
        reimburser_id: i64,
        amount: i64,
        received_at: i64,
        source_transaction_id: Option<&str>,
        reference: Option<&str>,
        note: Option<&str>,
        allocations: &[AllocationRequest<'_>],
    ) -> Result<Self> {
        if amount <= 0 {
            bail!("reimbursement payment amount must be positive");
        }
        let allocated = allocations.iter().try_fold(0_i64, |total, allocation| {
            if allocation.amount <= 0 {
                bail!("allocation amounts must be positive");
            }
            total
                .checked_add(allocation.amount)
                .context("allocation total overflow")
        })?;
        if allocated > amount {
            bail!("allocations exceed payment amount");
        }

        let now = Utc::now().timestamp();
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO reimbursement_payments
             (reimburser_id, amount, received_at, source_transaction_id, reference, note, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![reimburser_id, amount, received_at, source_transaction_id, reference, note, now],
        )?;
        let id = tx.last_insert_rowid();
        for allocation in allocations {
            allocate(&tx, id, reimburser_id, allocation, now)?;
        }
        tx.commit()?;

        Ok(Self {
            id,
            reimburser_id,
            amount,
            received_at,
            source_transaction_id: source_transaction_id.map(str::to_owned),
            reference: reference.map(str::to_owned),
            note: note.map(str::to_owned),
        })
    }

    pub fn unallocated_amount(conn: &Connection, payment_id: i64) -> Result<i64> {
        conn.query_row(
            "SELECT p.amount - COALESCE(SUM(a.amount), 0)
             FROM reimbursement_payments p
             LEFT JOIN reimbursement_allocations a ON a.payment_id = p.id
             WHERE p.id = ?1 GROUP BY p.id",
            [payment_id],
            |row| row.get(0),
        )
        .optional()?
        .with_context(|| format!("reimbursement payment {} not found", payment_id))
    }
}

fn allocate(
    conn: &Connection,
    payment_id: i64,
    reimburser_id: i64,
    allocation: &AllocationRequest<'_>,
    now: i64,
) -> Result<()> {
    let claim: Option<(i64, i64)> = conn
        .query_row(
            "SELECT reimburser_id, -amount FROM transactions WHERE id = ?1",
            [allocation.transaction_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let (claim_reimburser, claim_amount) =
        claim.with_context(|| format!("transaction '{}' not found", allocation.transaction_id))?;
    if claim_reimburser != reimburser_id {
        bail!(
            "transaction '{}' belongs to a different reimburser",
            allocation.transaction_id
        );
    }
    if claim_amount <= 0 {
        bail!(
            "transaction '{}' is not a reimbursable expense",
            allocation.transaction_id
        );
    }
    let already_allocated: i64 = conn.query_row(
        "SELECT COALESCE(SUM(amount), 0) FROM reimbursement_allocations WHERE transaction_id = ?1",
        [allocation.transaction_id],
        |row| row.get(0),
    )?;
    if allocation.amount > claim_amount - already_allocated {
        bail!(
            "allocation exceeds outstanding amount for transaction '{}'",
            allocation.transaction_id
        );
    }
    conn.execute(
        "INSERT INTO reimbursement_allocations (payment_id, transaction_id, amount, created_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            payment_id,
            allocation.transaction_id,
            allocation.amount,
            now
        ],
    )?;
    if already_allocated + allocation.amount == claim_amount {
        conn.execute(
            "UPDATE transactions SET reimbursed_at = ?1, updated_at = ?1 WHERE id = ?2",
            params![now, allocation.transaction_id],
        )?;
    }
    Ok(())
}

pub fn outstanding_amount(conn: &Connection, transaction_id: &str) -> Result<i64> {
    conn.query_row(
        "SELECT CASE
             WHEN t.reimbursed_at IS NOT NULL AND COUNT(a.id) = 0 THEN 0
             ELSE MAX(0, -t.amount - COALESCE(SUM(a.amount), 0))
         END
         FROM transactions t
         LEFT JOIN reimbursement_allocations a ON a.transaction_id = t.id
         WHERE t.id = ?1 AND t.reimburser_id IS NOT NULL
         GROUP BY t.id",
        [transaction_id],
        |row| row.get(0),
    )
    .optional()?
    .with_context(|| format!("reimbursable transaction '{}' not found", transaction_id))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgingSummary {
    pub reimburser_id: i64,
    pub reimburser: String,
    pub current: i64,
    pub days_31_60: i64,
    pub days_61_90: i64,
    pub over_90_days: i64,
    pub total: i64,
}

/// Outstanding reimbursement balances grouped into age buckets as of `as_of`.
pub fn aging_summary(conn: &Connection, as_of: i64) -> Result<Vec<AgingSummary>> {
    let mut stmt = conn.prepare(
        "WITH claims AS (
            SELECT t.id, t.reimburser_id, t.posted,
                   CASE WHEN t.reimbursed_at IS NOT NULL AND COUNT(a.id) = 0 THEN 0
                        ELSE MAX(0, -t.amount - COALESCE(SUM(a.amount), 0)) END outstanding
            FROM transactions t
            LEFT JOIN reimbursement_allocations a ON a.transaction_id = t.id
            WHERE t.reimburser_id IS NOT NULL
            GROUP BY t.id
        )
        SELECT r.id, r.name,
               SUM(CASE WHEN (?1 - c.posted) / 86400 <= 30 THEN outstanding ELSE 0 END),
               SUM(CASE WHEN (?1 - c.posted) / 86400 BETWEEN 31 AND 60 THEN outstanding ELSE 0 END),
               SUM(CASE WHEN (?1 - c.posted) / 86400 BETWEEN 61 AND 90 THEN outstanding ELSE 0 END),
               SUM(CASE WHEN (?1 - c.posted) / 86400 > 90 THEN outstanding ELSE 0 END),
               SUM(outstanding)
        FROM claims c JOIN reimbursers r ON r.id = c.reimburser_id
        GROUP BY r.id, r.name HAVING SUM(outstanding) > 0 ORDER BY r.name",
    )?;
    let rows = stmt.query_map([as_of], |row| {
        Ok(AgingSummary {
            reimburser_id: row.get(0)?,
            reimburser: row.get(1)?,
            current: row.get(2)?,
            days_31_60: row.get(3)?,
            days_61_90: row.get(4)?,
            over_90_days: row.get(5)?,
            total: row.get(6)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn fixture() -> Result<(Connection, i64)> {
        let conn = db::open_in_memory()?;
        migrate(&conn)?;
        conn.execute("INSERT INTO accounts (id, name, institution, account_type, created_at, updated_at) VALUES ('a', 'Card', 'Bank', 'credit', 0, 0)", [])?;
        conn.execute(
            "INSERT INTO reimbursers (name, created_at) VALUES ('Acme', 0)",
            [],
        )?;
        let rid = conn.last_insert_rowid();
        for (id, posted, amount) in [
            ("t1", 100_i64, -10_000_i64),
            ("t2", 100 + 40 * 86_400, -5_000),
        ] {
            conn.execute("INSERT INTO transactions (id, account_id, posted, amount, description, pending, reimburser_id, created_at, updated_at) VALUES (?1, 'a', ?2, ?3, 'Expense', 0, ?4, 0, 0)", params![id, posted, amount, rid])?;
        }
        Ok((conn, rid))
    }

    #[test]
    fn supports_partial_and_batch_allocations() -> Result<()> {
        let (conn, rid) = fixture()?;
        let payment = ReimbursementPayment::create(
            &conn,
            rid,
            12_000,
            300,
            None,
            Some("ACH-1"),
            None,
            &[
                AllocationRequest {
                    transaction_id: "t1",
                    amount: 10_000,
                },
                AllocationRequest {
                    transaction_id: "t2",
                    amount: 2_000,
                },
            ],
        )?;
        assert_eq!(
            ReimbursementPayment::unallocated_amount(&conn, payment.id)?,
            0
        );
        assert_eq!(outstanding_amount(&conn, "t1")?, 0);
        assert_eq!(outstanding_amount(&conn, "t2")?, 3_000);
        let paid: Option<i64> = conn.query_row(
            "SELECT reimbursed_at FROM transactions WHERE id = 't1'",
            [],
            |r| r.get(0),
        )?;
        assert!(paid.is_some());
        Ok(())
    }

    #[test]
    fn rejects_overallocation_atomically() -> Result<()> {
        let (conn, rid) = fixture()?;
        let error = ReimbursementPayment::create(
            &conn,
            rid,
            11_000,
            300,
            None,
            None,
            None,
            &[AllocationRequest {
                transaction_id: "t1",
                amount: 10_001,
            }],
        )
        .unwrap_err();
        assert!(error.to_string().contains("exceeds outstanding"));
        let count: i64 =
            conn.query_row("SELECT COUNT(*) FROM reimbursement_payments", [], |r| {
                r.get(0)
            })?;
        assert_eq!(count, 0);
        Ok(())
    }

    #[test]
    fn reports_aging_by_entity() -> Result<()> {
        let (conn, _rid) = fixture()?;
        let summary = aging_summary(&conn, 100 + 100 * 86_400)?;
        assert_eq!(summary.len(), 1);
        assert_eq!(summary[0].over_90_days, 10_000);
        assert_eq!(summary[0].days_31_60, 5_000);
        assert_eq!(summary[0].total, 15_000);
        Ok(())
    }
}
