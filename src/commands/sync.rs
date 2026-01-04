use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use rusqlite::OptionalExtension;

use crate::{db, rules, simplefin::SimpleFin};

use super::setup::get_access_url;

pub fn run(days: u32) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint init' first.")?;
    let access_url = get_access_url(&conn)?;

    let now = Utc::now();
    let start = now - Duration::days(days as i64);

    println!("Fetching transactions from the last {} days...", days);

    let client = SimpleFin::new(access_url);
    let account_set = client.fetch_accounts(Some(start.timestamp()), Some(now.timestamp()))?;

    let now_ts = now.timestamp();
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
            // Check if transaction exists by ID or by dedup key (account, posted, amount, description)
            let existing: Option<(String, bool)> = conn
                .query_row(
                    "SELECT id, pending FROM transactions
                     WHERE id = ?1
                        OR (account_id = ?2 AND posted = ?3 AND amount = ?4 AND description = ?5)",
                    rusqlite::params![
                        tx.id,
                        account.id,
                        tx.posted,
                        tx.amount_cents(),
                        tx.description,
                    ],
                    |row| Ok((row.get(0)?, row.get::<_, i64>(1)? != 0)),
                )
                .optional()?;

            match existing {
                Some((id, old_pending)) => {
                    // Only update if pending status changed
                    if old_pending != tx.pending {
                        conn.execute(
                            "UPDATE transactions SET pending = ?1, updated_at = ?2 WHERE id = ?3",
                            rusqlite::params![tx.pending as i64, now_ts, id],
                        )?;
                        transactions_updated += 1;
                    }
                    // Otherwise skip - transaction unchanged
                }
                None => {
                    // Insert new transaction
                    conn.execute(
                        "INSERT INTO transactions (id, account_id, posted, amount, description, pending, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
                        rusqlite::params![
                            tx.id,
                            account.id,
                            tx.posted,
                            tx.amount_cents(),
                            tx.description,
                            tx.pending as i64,
                            now_ts,
                        ],
                    )?;
                    transactions_inserted += 1;
                }
            }
        }
    }

    println!(
        "Synced {} accounts, {} new transactions, {} updated",
        accounts_updated, transactions_inserted, transactions_updated
    );

    // Auto-categorize new transactions
    let categorized = rules::auto_categorize_all(&conn)?;
    if categorized > 0 {
        println!("Auto-categorized {} transactions", categorized);
    }

    Ok(())
}
