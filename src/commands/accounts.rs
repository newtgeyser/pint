use anyhow::{bail, Context, Result};
use chrono::{TimeZone, Utc};
use rusqlite::OptionalExtension;

use crate::db::{self, models::Account};

const VALID_TYPES: &[&str] = &[
    "checking",
    "savings",
    "credit",
    "brokerage",
    "retirement",
    "loan",
    "money market",
    "unknown",
];

pub fn run() -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint init' first.")?;

    let mut stmt = conn.prepare(
        "SELECT id, name, institution, account_type, balance, balance_date, currency, created_at, updated_at
         FROM accounts
         ORDER BY account_type, name",
    )?;

    let accounts: Vec<Account> = stmt
        .query_map([], |row| Account::from_row(row))?
        .collect::<Result<Vec<_>, _>>()?;

    if accounts.is_empty() {
        println!("No accounts found. Run 'pint sync' to fetch accounts.");
        return Ok(());
    }

    println!("{:<12} {:<12} {:<28} {:>12} {:>4}  {}", "ID", "TYPE", "ACCOUNT", "BALANCE", "CUR", "AS OF");
    println!("{}", "-".repeat(86));

    for account in accounts {
        let balance_str = account
            .balance_dollars()
            .map(|b| format!("{:>12.2}", b))
            .unwrap_or_else(|| "         N/A".to_string());

        let date_str = account
            .balance_date
            .map(|ts| {
                Utc.timestamp_opt(ts, 0)
                    .single()
                    .map(|dt| dt.format("%Y-%m-%d").to_string())
                    .unwrap_or_else(|| "?".to_string())
            })
            .unwrap_or_default();

        let display_name = if let Some(inst) = &account.institution {
            format!("{} ({})", account.name, inst)
        } else {
            account.name.clone()
        };

        let short_id = truncate(&account.id, 12);

        println!(
            "{:<12} {:<12} {:<28} {} {:>4}  {}",
            short_id,
            account.account_type,
            truncate(&display_name, 28),
            balance_str,
            account.currency,
            date_str,
        );
    }

    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max - 3])
    }
}

pub fn set_type(account_query: &str, account_type: &str) -> Result<()> {
    let account_type = account_type.to_lowercase();
    if !VALID_TYPES.contains(&account_type.as_str()) {
        bail!(
            "Invalid account type '{}'. Valid types: {}",
            account_type,
            VALID_TYPES.join(", ")
        );
    }

    let conn = db::open().context("Database not found. Run 'pint init' first.")?;

    // Find account by ID (exact or prefix) or name (partial match)
    let account: Option<(String, String)> = conn
        .query_row(
            "SELECT id, name FROM accounts
             WHERE id = ?1 OR id LIKE ?1 || '%' OR name LIKE '%' || ?1 || '%'
             LIMIT 1",
            [account_query],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;

    match account {
        Some((id, name)) => {
            conn.execute(
                "UPDATE accounts SET account_type = ?1, updated_at = ?2 WHERE id = ?3",
                rusqlite::params![account_type, chrono::Utc::now().timestamp(), id],
            )?;
            println!("Set account '{}' type to '{}'", name, account_type);
            Ok(())
        }
        None => bail!("No account found matching '{}'", account_query),
    }
}
