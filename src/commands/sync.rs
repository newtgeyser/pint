use anyhow::{Context, Result};
use chrono::{Duration, NaiveDate, TimeZone, Utc};
use rusqlite::Connection;

use crate::{db, simplefin::{SimpleFin, AccountSet}};
use super::{prices, rules};

use super::setup::get_access_url;

const BACKFILL_CHUNK_DAYS: i64 = 60;
const BACKFILL_MAX_REQUESTS: usize = 12;

pub struct SyncStats {
    pub accounts: usize,
    pub inserted: usize,
    pub modified: usize,
    pub holdings: usize,
    pub warnings: Vec<String>,
}

impl SyncStats {
    pub fn summary(&self) -> String {
        let tx_word = if self.inserted == 1 { "transaction" } else { "transactions" };
        let mut summary = format!(
            "Synced {} accounts, {} new {}",
            self.accounts, self.inserted, tx_word
        );
        if self.modified > 0 {
            summary.push_str(&format!(", {} modified", self.modified));
        }
        if self.holdings > 0 {
            summary.push_str(&format!(", {} holdings", self.holdings));
        }
        summary
    }
}

pub fn run(days: u32) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint init' first.")?;
    println!("Fetching transactions from the last {} days...", days);
    let stats = run_with_conn(&conn, days)?;
    println!("{}", stats.summary());
    if !stats.warnings.is_empty() {
        eprintln!("SimpleFIN warnings:");
        for warning in &stats.warnings {
            eprintln!("  - {}", warning);
        }
    }
    auto_categorize(&conn)?;
    update_manual_prices(&conn)?;
    Ok(())
}

/// Run sync with an existing connection, returning stats without printing.
pub fn run_with_conn(conn: &Connection, days: u32) -> Result<SyncStats> {
    let access_url = get_access_url(conn)?;
    let client = SimpleFin::new(access_url)?;

    let now = Utc::now();
    let start = now - Duration::days(days as i64);

    let account_set = client.fetch_accounts(Some(start.timestamp()), Some(now.timestamp()))?;
    let warnings = account_set.errors.clone();
    let mut stats = import_accounts(conn, &account_set)?;
    stats.warnings = warnings;

    // Auto-categorize
    rules::auto_categorize_all(conn)?;

    Ok(stats)
}

pub fn run_backfill(from: Option<NaiveDate>) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint init' first.")?;
    let access_url = get_access_url(&conn)?;
    let client = SimpleFin::new(access_url)?;

    let mut end = match from {
        Some(date) => Utc.from_utc_datetime(&date.and_hms_opt(23, 59, 59).unwrap()),
        None => Utc::now(),
    };
    let mut total_inserted = 0;
    let mut total_modified = 0;
    let mut requests = 0;

    match from {
        Some(date) => println!(
            "Backfilling from {} (up to {} requests, {} days each)...",
            date, BACKFILL_MAX_REQUESTS, BACKFILL_CHUNK_DAYS
        ),
        None => println!(
            "Backfilling transactions (up to {} requests, {} days each)...",
            BACKFILL_MAX_REQUESTS, BACKFILL_CHUNK_DAYS
        ),
    }

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
        let returned_tx_count: usize = account_set
            .accounts
            .iter()
            .map(|a| a.transactions.len())
            .sum();

        if returned_tx_count == 0 {
            println!("  No transaction data returned, stopping backfill.");
            break;
        }

        let stats = import_accounts(&conn, &account_set)?;

        println!(
            "    -> {} new, {} modified",
            stats.inserted, stats.modified
        );

        total_inserted += stats.inserted;
        total_modified += stats.modified;

        // Move window back
        end = start;
    }

    println!(
        "Backfill complete: {} requests, {} new transactions, {} modified",
        requests, total_inserted, total_modified
    );

    auto_categorize(&conn)?;
    update_manual_prices(&conn)?;
    Ok(())
}

fn import_accounts(conn: &Connection, account_set: &AccountSet) -> Result<SyncStats> {
    let now_ts = Utc::now().timestamp();
    let mut accounts_updated = 0;
    let mut transactions_inserted = 0;
    let mut transactions_modified = 0;
    let mut holdings_updated = 0;

    for account in &account_set.accounts {
        let institution = account.institution_name();

        // Determine account type: use detected type, or upgrade to brokerage if has holdings
        let account_type = account.account_type();

        conn.execute(
            "INSERT INTO accounts (id, name, institution, account_type, balance, balance_date, currency, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                institution = excluded.institution,
                account_type = CASE
                    WHEN accounts.account_type = 'unknown' THEN excluded.account_type
                    ELSE accounts.account_type
                END,
                balance = excluded.balance,
                balance_date = excluded.balance_date,
                currency = excluded.currency,
                updated_at = excluded.updated_at",
            rusqlite::params![
                account.id,
                account.name,
                institution,
                account_type,
                account.balance_cents(),
                account.balance_date,
                account.currency.as_deref().unwrap_or("USD"),
                now_ts,
            ],
        )?;
        accounts_updated += 1;

        for tx in &account.transactions {
            let (inserted, modified) = import_transaction(conn, &account.id, tx, now_ts)?;
            transactions_inserted += inserted;
            transactions_modified += modified;
        }

        // Import holdings
        for holding in &account.holdings {
            import_holding(conn, &account.id, holding, now_ts)?;
            holdings_updated += 1;
        }
    }

    Ok(SyncStats {
        accounts: accounts_updated,
        inserted: transactions_inserted,
        modified: transactions_modified,
        holdings: holdings_updated,
        warnings: Vec::new(), // Populated by caller with account_set.errors
    })
}

/// Returns (inserted, modified) - modified only if data actually changed
fn import_transaction(
    conn: &Connection,
    account_id: &str,
    tx: &crate::simplefin::Transaction,
    now_ts: i64,
) -> Result<(usize, usize)> {
    use rusqlite::OptionalExtension;

    let amount_cents = tx.amount_cents()?;
    let new_pending = tx.pending as i64;

    // Check if transaction exists and if key fields differ
    let existing: Option<(i64, i64, String, i64)> = conn
        .query_row(
            "SELECT posted, amount, description, pending FROM transactions WHERE id = ?1",
            [tx.id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;

    let (is_new, is_modified) = match existing {
        None => (true, false),
        Some((old_posted, old_amount, old_desc, old_pending)) => {
            let changed = old_posted != tx.posted
                || old_amount != amount_cents
                || old_desc != tx.description
                || old_pending != new_pending;
            (false, changed)
        }
    };

    conn.execute(
        "INSERT INTO transactions (id, account_id, posted, amount, description, pending, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
         ON CONFLICT(id) DO UPDATE SET
            account_id = excluded.account_id,
            posted = excluded.posted,
            amount = excluded.amount,
            description = excluded.description,
            pending = excluded.pending,
            updated_at = excluded.updated_at",
        rusqlite::params![
            tx.id,
            account_id,
            tx.posted,
            amount_cents,
            tx.description,
            new_pending,
            now_ts,
        ],
    )?;

    Ok((is_new as usize, is_modified as usize))
}

fn import_holding(
    conn: &Connection,
    account_id: &str,
    holding: &crate::simplefin::Holding,
    now_ts: i64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO holdings (id, account_id, symbol, description, shares, price, cost_basis, market_value, currency, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)
         ON CONFLICT(id) DO UPDATE SET
            account_id = excluded.account_id,
            symbol = excluded.symbol,
            description = excluded.description,
            shares = excluded.shares,
            price = excluded.price,
            cost_basis = excluded.cost_basis,
            market_value = excluded.market_value,
            currency = excluded.currency,
            updated_at = excluded.updated_at",
        rusqlite::params![
            holding.id,
            account_id,
            holding.symbol,
            holding.description,
            holding.shares.as_deref().unwrap_or("0"),
            holding.price_cents(),
            holding.cost_basis_cents(),
            holding.market_value_cents(),
            holding.currency.as_deref().unwrap_or("USD"),
            now_ts,
        ],
    )?;
    Ok(())
}

fn auto_categorize(conn: &Connection) -> Result<()> {
    let categorized = rules::auto_categorize_all(conn)?;
    if categorized > 0 {
        println!("Auto-categorized {} transactions", categorized);
    }
    Ok(())
}

fn update_manual_prices(conn: &Connection) -> Result<()> {
    let stats = prices::run_with_conn(conn)?;
    if stats.updated > 0 {
        println!("Updated {} holding prices from Yahoo Finance", stats.updated);
    }
    Ok(())
}
