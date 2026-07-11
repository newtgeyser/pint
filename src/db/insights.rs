use anyhow::Result;
use chrono::Utc;
use rusqlite::Connection;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NetWorthSnapshot {
    pub snapshot_date: String,
    pub captured_at: i64,
    pub cash: i64,
    pub brokerage: i64,
    pub retirement: i64,
    pub assets: i64,
    pub credit: i64,
    pub net_worth: i64,
}

impl NetWorthSnapshot {
    pub fn current(conn: &Connection) -> Result<Self> {
        let (cash, brokerage, retirement, credit) = conn.query_row(
            "SELECT
                COALESCE(SUM(CASE WHEN account_type IN ('checking', 'savings') THEN balance ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN account_type = 'brokerage' THEN balance ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN account_type = 'retirement' THEN balance ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN account_type = 'credit' THEN balance ELSE 0 END), 0)
             FROM accounts",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
        let assets = conn.query_row("SELECT COALESCE(SUM(value), 0) FROM assets", [], |row| {
            row.get(0)
        })?;
        let captured_at = Utc::now().timestamp();
        Ok(Self {
            snapshot_date: Utc::now().format("%Y-%m-%d").to_string(),
            captured_at,
            cash,
            brokerage,
            retirement,
            assets,
            credit,
            net_worth: cash + brokerage + retirement + assets + credit,
        })
    }

    pub fn capture(conn: &Connection) -> Result<Self> {
        let snapshot = Self::current(conn)?;
        conn.execute(
            "INSERT INTO net_worth_snapshots
                (snapshot_date, captured_at, cash, brokerage, retirement, assets, credit, net_worth)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(snapshot_date) DO UPDATE SET
                captured_at = excluded.captured_at,
                cash = excluded.cash,
                brokerage = excluded.brokerage,
                retirement = excluded.retirement,
                assets = excluded.assets,
                credit = excluded.credit,
                net_worth = excluded.net_worth",
            rusqlite::params![
                snapshot.snapshot_date,
                snapshot.captured_at,
                snapshot.cash,
                snapshot.brokerage,
                snapshot.retirement,
                snapshot.assets,
                snapshot.credit,
                snapshot.net_worth,
            ],
        )?;
        Ok(snapshot)
    }

    pub fn history(conn: &Connection, limit: usize) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare(
            "SELECT snapshot_date, captured_at, cash, brokerage, retirement, assets, credit, net_worth
             FROM net_worth_snapshots ORDER BY snapshot_date DESC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map([limit as i64], |row| {
                Ok(Self {
                    snapshot_date: row.get(0)?,
                    captured_at: row.get(1)?,
                    cash: row.get(2)?,
                    brokerage: row.get(3)?,
                    retirement: row.get(4)?,
                    assets: row.get(5)?,
                    credit: row.get(6)?,
                    net_worth: row.get(7)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataHealth {
    pub stale_accounts: i64,
    pub holdings_without_price: i64,
    pub holdings_without_cost_basis: i64,
    pub assets_without_value: i64,
    pub uncategorized_transactions: i64,
    pub pending_reimbursements_over_30_days: i64,
}

impl DataHealth {
    pub fn load(conn: &Connection) -> Result<Self> {
        let stale_cutoff = Utc::now().timestamp() - 7 * 86_400;
        let reimbursement_cutoff = Utc::now().timestamp() - 30 * 86_400;
        Ok(Self {
            stale_accounts: conn.query_row(
                "SELECT COUNT(*) FROM accounts WHERE balance_date IS NULL OR balance_date < ?1",
                [stale_cutoff],
                |row| row.get(0),
            )?,
            holdings_without_price: conn.query_row(
                "SELECT COUNT(*) FROM holdings WHERE price IS NULL",
                [],
                |row| row.get(0),
            )?,
            holdings_without_cost_basis: conn.query_row(
                "SELECT COUNT(*) FROM holdings WHERE cost_basis IS NULL",
                [],
                |row| row.get(0),
            )?,
            assets_without_value: conn.query_row(
                "SELECT COUNT(*) FROM assets WHERE value IS NULL",
                [],
                |row| row.get(0),
            )?,
            uncategorized_transactions: conn.query_row(
                "SELECT COUNT(*) FROM transactions WHERE category_id IS NULL",
                [],
                |row| row.get(0),
            )?,
            pending_reimbursements_over_30_days: conn.query_row(
                "SELECT COUNT(*) FROM transactions
                 WHERE reimburser_id IS NOT NULL AND reimbursed_at IS NULL AND posted < ?1",
                [reimbursement_cutoff],
                |row| row.get(0),
            )?,
        })
    }

    pub fn issue_count(&self) -> i64 {
        self.stale_accounts
            + self.holdings_without_price
            + self.holdings_without_cost_basis
            + self.assets_without_value
            + self.uncategorized_transactions
            + self.pending_reimbursements_over_30_days
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn captures_one_snapshot_per_day() {
        let conn = db::open_in_memory().unwrap();
        let first = NetWorthSnapshot::capture(&conn).unwrap();
        NetWorthSnapshot::capture(&conn).unwrap();
        let history = NetWorthSnapshot::history(&conn, 10).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].snapshot_date, first.snapshot_date);
    }
}
