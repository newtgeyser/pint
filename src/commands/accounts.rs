use anyhow::{bail, Context, Result};
use chrono::{TimeZone, Utc};
use rusqlite::OptionalExtension;
use uuid::Uuid;

use crate::db::{self, models::Account};
use crate::util::truncate;

pub const VALID_TYPES: &[&str] = &[
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
        "SELECT id, name, nickname, institution, account_type, balance, balance_date, currency, manual, created_at, updated_at
         FROM accounts
         ORDER BY manual DESC, account_type, COALESCE(nickname, name)",
    )?;

    let accounts: Vec<Account> = stmt
        .query_map([], |row| Account::from_row(row))?
        .collect::<Result<Vec<_>, _>>()?;

    if accounts.is_empty() {
        println!("No accounts found. Run 'pint sync' to fetch accounts or 'pint accounts add' to create one.");
        return Ok(());
    }

    println!("{:<12} {:<14} {:<28} {:>12} {:>4}  {}", "ID", "TYPE", "ACCOUNT", "BALANCE", "CUR", "AS OF");
    println!("{}", "-".repeat(88));

    let mut total_balance = 0i64;

    for account in &accounts {
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

        let display_name = account.display_name();

        let short_id = truncate(&account.id, 12);

        // Mark manual accounts with "*"
        let type_display = if account.manual {
            format!("{}*", account.account_type)
        } else {
            account.account_type.clone()
        };

        println!(
            "{:<12} {:<14} {:<28} {} {:>4}  {}",
            short_id,
            type_display,
            truncate(display_name, 28),
            balance_str,
            account.currency,
            date_str,
        );

        if let Some(b) = account.balance {
            total_balance += b;
        }
    }

    println!("{}", "-".repeat(88));
    println!(
        "{:<12} {:<14} {:<28} {:>12.2}",
        "",
        "",
        "TOTAL",
        total_balance as f64 / 100.0,
    );

    println!("\n* = manual account");

    Ok(())
}

pub fn add(name: &str, account_type: &str) -> Result<()> {
    let account_type = account_type.to_lowercase();
    if !VALID_TYPES.contains(&account_type.as_str()) {
        bail!(
            "Invalid account type '{}'. Valid types: {}",
            account_type,
            VALID_TYPES.join(", ")
        );
    }

    let conn = db::open().context("Database not found. Run 'pint init' first.")?;

    let id = format!("manual-{}", Uuid::new_v4());
    let now = Utc::now().timestamp();

    conn.execute(
        "INSERT INTO accounts (id, name, account_type, currency, manual, created_at, updated_at)
         VALUES (?1, ?2, ?3, 'USD', 1, ?4, ?4)",
        rusqlite::params![id, name, account_type, now],
    )?;

    println!("Created manual account '{}' (ID: {})", name, truncate(&id, 12));
    Ok(())
}

pub fn remove(account_query: &str) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint init' first.")?;

    // Find account by ID (exact or prefix), nickname, or name (partial match)
    let account: Option<(String, String)> = conn
        .query_row(
            "SELECT id, COALESCE(nickname, name) FROM accounts
             WHERE id = ?1 OR id LIKE ?1 || '%' OR nickname LIKE '%' || ?1 || '%' OR name LIKE '%' || ?1 || '%'
             LIMIT 1",
            [account_query],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;

    match account {
        Some((id, name)) => {
            // Delete associated transactions first
            let txns_deleted = conn.execute(
                "DELETE FROM transactions WHERE account_id = ?1",
                [&id],
            )?;

            // Delete associated holdings
            let holdings_deleted = conn.execute(
                "DELETE FROM holdings WHERE account_id = ?1",
                [&id],
            )?;

            conn.execute("DELETE FROM accounts WHERE id = ?1", [&id])?;

            let mut parts = vec![format!("Removed account '{}'", name)];
            if txns_deleted > 0 {
                parts.push(format!("{} transactions", txns_deleted));
            }
            if holdings_deleted > 0 {
                parts.push(format!("{} holdings", holdings_deleted));
            }
            if parts.len() > 1 {
                println!("{} and {}", parts[0], parts[1..].join(", "));
            } else {
                println!("{}", parts[0]);
            }
            Ok(())
        }
        None => bail!("No account found matching '{}'", account_query),
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

    // Find account by ID (exact or prefix), nickname, or name (partial match)
    let account: Option<(String, String)> = conn
        .query_row(
            "SELECT id, COALESCE(nickname, name) FROM accounts
             WHERE id = ?1 OR id LIKE ?1 || '%' OR nickname LIKE '%' || ?1 || '%' OR name LIKE '%' || ?1 || '%'
             LIMIT 1",
            [account_query],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;

    match account {
        Some((id, name)) => {
            conn.execute(
                "UPDATE accounts SET account_type = ?1, updated_at = ?2 WHERE id = ?3",
                rusqlite::params![account_type, Utc::now().timestamp(), id],
            )?;
            println!("Set account '{}' type to '{}'", name, account_type);
            Ok(())
        }
        None => bail!("No account found matching '{}'", account_query),
    }
}

pub fn set_nickname(account_query: &str, nickname: Option<&str>) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint init' first.")?;

    // Find account by ID (exact or prefix), nickname, or name (partial match)
    let account: Option<(String, String)> = conn
        .query_row(
            "SELECT id, COALESCE(nickname, name) FROM accounts
             WHERE id = ?1 OR id LIKE ?1 || '%' OR nickname LIKE '%' || ?1 || '%' OR name LIKE '%' || ?1 || '%'
             LIMIT 1",
            [account_query],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;

    match account {
        Some((id, current_name)) => {
            conn.execute(
                "UPDATE accounts SET nickname = ?1, updated_at = ?2 WHERE id = ?3",
                rusqlite::params![nickname, Utc::now().timestamp(), id],
            )?;
            match nickname {
                Some(n) => println!("Set account '{}' nickname to '{}'", current_name, n),
                None => println!("Cleared nickname for account '{}'", current_name),
            }
            Ok(())
        }
        None => bail!("No account found matching '{}'", account_query),
    }
}
