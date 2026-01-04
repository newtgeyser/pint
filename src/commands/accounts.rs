use anyhow::{Context, Result};
use chrono::{TimeZone, Utc};

use crate::db::{self, models::Account};

pub fn run() -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint init' first.")?;

    let mut stmt = conn.prepare(
        "SELECT id, name, institution, balance, balance_date, currency, created_at, updated_at
         FROM accounts
         ORDER BY name",
    )?;

    let accounts: Vec<Account> = stmt
        .query_map([], |row| Account::from_row(row))?
        .collect::<Result<Vec<_>, _>>()?;

    if accounts.is_empty() {
        println!("No accounts found. Run 'pint sync' to fetch accounts.");
        return Ok(());
    }

    println!("{:<40} {:>12} {:>6}  {}", "ACCOUNT", "BALANCE", "CUR", "AS OF");
    println!("{}", "-".repeat(72));

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

        println!(
            "{:<40} {} {:>6}  {}",
            truncate(&display_name, 40),
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
