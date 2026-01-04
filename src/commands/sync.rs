use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use rusqlite::{Connection, OptionalExtension};

use crate::{db, rules, simplefin::{SimpleFin, AccountSet}};

use super::setup::get_access_url;

const BACKFILL_CHUNK_DAYS: i64 = 60;
const BACKFILL_MAX_REQUESTS: usize = 12;

struct SyncStats {
    accounts: usize,
    inserted: usize,
    updated: usize,
}

pub fn run(days: u32) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint init' first.")?;
    let access_url = get_access_url(&conn)?;
    let client = SimpleFin::new(access_url);

    let now = Utc::now();
    let start = now - Duration::days(days as i64);

    println!("Fetching transactions from the last {} days...", days);

    let account_set = client.fetch_accounts(Some(start.timestamp()), Some(now.timestamp()))?;
    let stats = import_accounts(&conn, &account_set)?;

    println!(
        "Synced {} accounts, {} new transactions, {} updated",
        stats.accounts, stats.inserted, stats.updated
    );

    auto_categorize(&conn)?;
    Ok(())
}

pub fn run_backfill() -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint init' first.")?;
    let access_url = get_access_url(&conn)?;
    let client = SimpleFin::new(access_url);

    let now = Utc::now();
    let mut end = now;
    let mut total_inserted = 0;
    let mut total_updated = 0;
    let mut requests = 0;

    println!(
        "Backfilling transactions (up to {} requests, {} days each)...",
        BACKFILL_MAX_REQUESTS, BACKFILL_CHUNK_DAYS
    );

    while requests < BACKFILL_MAX_REQUESTS {
        let start = end - Duration::days(BACKFILL_CHUNK_DAYS);
        requests += 1;

        println!(
            "  Request {}/{}: {} to {}",
            requests,
            BACKFILL_MAX_REQUESTS,
            start.format("%Y-%m-%d"),
            end.format("%Y-%m-%d")
        );

        let account_set = client.fetch_accounts(Some(start.timestamp()), Some(end.timestamp()))?;
        let stats = import_accounts(&conn, &account_set)?;

        println!(
            "    -> {} new, {} updated",
            stats.inserted, stats.updated
        );

        total_inserted += stats.inserted;
        total_updated += stats.updated;

        // Stop if no new transactions (institution likely not providing older data)
        if stats.inserted == 0 {
            println!("  No new transactions found, stopping backfill.");
            break;
        }

        // Move window back
        end = start;
    }

    println!(
        "Backfill complete: {} requests, {} new transactions, {} updated",
        requests, total_inserted, total_updated
    );

    auto_categorize(&conn)?;
    Ok(())
}

fn import_accounts(conn: &Connection, account_set: &AccountSet) -> Result<SyncStats> {
    let now_ts = Utc::now().timestamp();
    let mut accounts_updated = 0;
    let mut transactions_inserted = 0;
    let mut transactions_updated = 0;

    for account in &account_set.accounts {
        let institution = account.institution_name();

        conn.execute(
            "INSERT INTO accounts (id, name, institution, balance, balance_date, currency, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                institution = excluded.institution,
                balance = excluded.balance,
                balance_date = excluded.balance_date,
                currency = excluded.currency,
                updated_at = excluded.updated_at",
            rusqlite::params![
                account.id,
                account.name,
                institution,
                account.balance_cents(),
                account.balance_date,
                account.currency.as_deref().unwrap_or("USD"),
                now_ts,
            ],
        )?;
        accounts_updated += 1;

        for tx in &account.transactions {
            let (inserted, updated) = import_transaction(conn, &account.id, tx, now_ts)?;
            transactions_inserted += inserted;
            transactions_updated += updated;
        }
    }

    Ok(SyncStats {
        accounts: accounts_updated,
        inserted: transactions_inserted,
        updated: transactions_updated,
    })
}

fn import_transaction(
    conn: &Connection,
    account_id: &str,
    tx: &crate::simplefin::Transaction,
    now_ts: i64,
) -> Result<(usize, usize)> {
    // Check if transaction exists by ID or by dedup key
    let existing: Option<(String, bool)> = conn
        .query_row(
            "SELECT id, pending FROM transactions
             WHERE id = ?1
                OR (account_id = ?2 AND posted = ?3 AND amount = ?4 AND description = ?5)",
            rusqlite::params![
                tx.id,
                account_id,
                tx.posted,
                tx.amount_cents(),
                tx.description,
            ],
            |row| Ok((row.get(0)?, row.get::<_, i64>(1)? != 0)),
        )
        .optional()?;

    match existing {
        Some((id, old_pending)) => {
            if old_pending != tx.pending {
                conn.execute(
                    "UPDATE transactions SET pending = ?1, updated_at = ?2 WHERE id = ?3",
                    rusqlite::params![tx.pending as i64, now_ts, id],
                )?;
                Ok((0, 1))
            } else {
                Ok((0, 0))
            }
        }
        None => {
            conn.execute(
                "INSERT INTO transactions (id, account_id, posted, amount, description, pending, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
                rusqlite::params![
                    tx.id,
                    account_id,
                    tx.posted,
                    tx.amount_cents(),
                    tx.description,
                    tx.pending as i64,
                    now_ts,
                ],
            )?;
            Ok((1, 0))
        }
    }
}

fn auto_categorize(conn: &Connection) -> Result<()> {
    let categorized = rules::auto_categorize_all(conn)?;
    if categorized > 0 {
        println!("Auto-categorized {} transactions", categorized);
    }
    Ok(())
}
