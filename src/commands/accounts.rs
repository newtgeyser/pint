use anyhow::{bail, Context, Result};
use chrono::{TimeZone, Utc};
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

    let accounts = Account::find_all(&conn)?;

    if accounts.is_empty() {
        println!("No accounts found. Run 'pint sync' to fetch accounts or 'pint accounts add' to create one.");
        return Ok(());
    }

    println!("{:<12} {:<14} {:<28} {:>12} {:>4}  AS OF", "ID", "TYPE", "ACCOUNT", "BALANCE", "CUR");
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
    add_quiet(name, account_type, false)
}

pub fn add_quiet(name: &str, account_type: &str, quiet: bool) -> Result<()> {
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

    if !quiet {
        println!("Created manual account '{}' (ID: {})", name, truncate(&id, 12));
    }
    Ok(())
}

pub fn remove(account_query: &str) -> Result<()> {
    remove_quiet(account_query, false)
}

pub fn remove_quiet(account_query: &str, quiet: bool) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint init' first.")?;

    match Account::find_by_query(&conn, account_query)? {
        Some(account) => {
            let id = &account.id;
            let name = account.display_name();
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

            if !quiet {
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
            }
            Ok(())
        }
        None => bail!("No account found matching '{}'", account_query),
    }
}


pub fn set_type(account_query: &str, account_type: &str) -> Result<()> {
    set_type_quiet(account_query, account_type, false)
}

pub fn set_type_quiet(account_query: &str, account_type: &str, quiet: bool) -> Result<()> {
    let account_type = account_type.to_lowercase();
    if !VALID_TYPES.contains(&account_type.as_str()) {
        bail!(
            "Invalid account type '{}'. Valid types: {}",
            account_type,
            VALID_TYPES.join(", ")
        );
    }

    let conn = db::open().context("Database not found. Run 'pint init' first.")?;

    match Account::find_by_query(&conn, account_query)? {
        Some(account) => {
            conn.execute(
                "UPDATE accounts SET account_type = ?1, updated_at = ?2 WHERE id = ?3",
                rusqlite::params![account_type, Utc::now().timestamp(), account.id],
            )?;
            if !quiet {
                println!("Set account '{}' type to '{}'", account.display_name(), account_type);
            }
            Ok(())
        }
        None => bail!("No account found matching '{}'", account_query),
    }
}

pub fn set_nickname(account_query: &str, nickname: Option<&str>) -> Result<()> {
    set_nickname_quiet(account_query, nickname, false)
}

pub fn set_nickname_quiet(account_query: &str, nickname: Option<&str>, quiet: bool) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint init' first.")?;

    match Account::find_by_query(&conn, account_query)? {
        Some(account) => {
            let current_name = account.display_name().to_string();
            conn.execute(
                "UPDATE accounts SET nickname = ?1, updated_at = ?2 WHERE id = ?3",
                rusqlite::params![nickname, Utc::now().timestamp(), account.id],
            )?;
            if !quiet {
                match nickname {
                    Some(n) => println!("Set account '{}' nickname to '{}'", current_name, n),
                    None => println!("Cleared nickname for account '{}'", current_name),
                }
            }
            Ok(())
        }
        None => bail!("No account found matching '{}'", account_query),
    }
}
