use anyhow::{bail, Context, Result};
use chrono::Utc;
use rusqlite::OptionalExtension;
use std::path::Path;

use crate::db;

pub fn run(file_path: &Path, account_query: &str) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint init' first.")?;

    // Find the account by ID, nickname, or name
    let account: Option<(String, String)> = conn
        .query_row(
            "SELECT id, COALESCE(nickname, name) FROM accounts
             WHERE id = ?1 OR id LIKE ?1 || '%' OR nickname LIKE '%' || ?1 || '%' OR name LIKE '%' || ?1 || '%'
             LIMIT 1",
            [account_query],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;

    let (account_id, account_name) = match account {
        Some(a) => a,
        None => bail!("No account found matching '{}'", account_query),
    };

    // Read and parse CSV
    let mut reader = csv::Reader::from_path(file_path)
        .with_context(|| format!("Failed to open {}", file_path.display()))?;

    let now_ts = Utc::now().timestamp();
    let mut imported = 0;
    let mut skipped = 0;

    // Delete existing holdings for this account
    let deleted = conn.execute(
        "DELETE FROM holdings WHERE account_id = ?1",
        [&account_id],
    )?;

    for result in reader.records() {
        let record = result.context("Failed to parse CSV record")?;

        // Columns: Holding, CUSIPNO, Symbol, Quantity, Price, As of, Value, Cost, Gain/Loss, % of Total
        let description = record.get(0).unwrap_or("").trim();
        let cusip = record.get(1).unwrap_or("").trim();
        let symbol = record.get(2).unwrap_or("").trim();
        let quantity = record.get(3).unwrap_or("").trim();
        let price = record.get(4).unwrap_or("").trim();
        let market_value = record.get(6).unwrap_or("").trim();
        let cost_basis = record.get(7).unwrap_or("").trim();

        // Skip rows without meaningful data
        if symbol.is_empty() && cusip.is_empty() {
            skipped += 1;
            continue;
        }

        // Parse numeric values
        let shares = parse_quantity(quantity);
        let price_cents = parse_dollar_amount(price);
        let market_value_cents = parse_dollar_amount(market_value);
        let cost_basis_cents = parse_dollar_amount(cost_basis);

        // Generate a unique ID based on account + CUSIP (or symbol if no CUSIP)
        let holding_id = if !cusip.is_empty() {
            format!("{}:{}", account_id, cusip)
        } else {
            format!("{}:{}", account_id, symbol)
        };

        let symbol_val = if symbol.is_empty() { None } else { Some(symbol) };
        let desc_val = if description.is_empty() { None } else { Some(description) };

        conn.execute(
            "INSERT INTO holdings (id, account_id, symbol, description, shares, price, cost_basis, market_value, currency, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'USD', ?9, ?9)
             ON CONFLICT(id) DO UPDATE SET
                symbol = excluded.symbol,
                description = excluded.description,
                shares = excluded.shares,
                price = excluded.price,
                cost_basis = excluded.cost_basis,
                market_value = excluded.market_value,
                updated_at = excluded.updated_at",
            rusqlite::params![
                holding_id,
                account_id,
                symbol_val,
                desc_val,
                shares,
                price_cents,
                cost_basis_cents,
                market_value_cents,
                now_ts,
            ],
        )?;

        imported += 1;
    }

    // Update account balance to sum of holdings
    conn.execute(
        "UPDATE accounts SET
            balance = (SELECT COALESCE(SUM(market_value), 0) FROM holdings WHERE account_id = ?1),
            balance_date = ?2,
            updated_at = ?2
         WHERE id = ?1",
        rusqlite::params![account_id, now_ts],
    )?;

    if deleted > 0 {
        println!("Removed {} existing holdings from '{}'", deleted, account_name);
    }
    println!("Imported {} holdings into '{}' ({} skipped)", imported, account_name, skipped);

    Ok(())
}

/// Parse a quantity string like "1282.00" or "18199.970" into a string
fn parse_quantity(s: &str) -> String {
    s.replace(',', "").trim().to_string()
}

/// Parse a dollar amount like "$31,844.88" or "-$543.15" into cents
fn parse_dollar_amount(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.is_empty() || s == "0.0" {
        return None;
    }

    // Handle negative values like "-$543.15"
    let (sign, rest) = if let Some(stripped) = s.strip_prefix("-$") {
        (-1i64, stripped)
    } else if let Some(stripped) = s.strip_prefix('-') {
        (-1i64, stripped)
    } else if let Some(stripped) = s.strip_prefix('$') {
        (1i64, stripped)
    } else {
        (1i64, s)
    };

    // Remove commas and parse
    let clean: String = rest.chars().filter(|c| *c != ',').collect();

    let value: f64 = clean.parse().ok()?;
    Some((value * 100.0).round() as i64 * sign)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_dollar_amount() {
        assert_eq!(parse_dollar_amount("$31,844.88"), Some(3184488));
        assert_eq!(parse_dollar_amount("-$543.15"), Some(-54315));
        assert_eq!(parse_dollar_amount("$790.89"), Some(79089));
        assert_eq!(parse_dollar_amount("0.0"), None);
        assert_eq!(parse_dollar_amount(""), None);
        assert_eq!(parse_dollar_amount("$1,234.56"), Some(123456));
    }
}
