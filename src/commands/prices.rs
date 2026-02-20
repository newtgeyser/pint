//! Fetch and update holding prices from Yahoo Finance.

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::Connection;
use std::collections::HashMap;

use crate::db;

/// Result of a price update operation
pub struct PriceUpdateStats {
    pub updated: usize,
    pub failed: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

impl PriceUpdateStats {
    pub fn summary(&self) -> String {
        let mut parts = vec![format!("{} updated", self.updated)];
        if self.failed > 0 {
            parts.push(format!("{} failed", self.failed));
        }
        if self.skipped > 0 {
            parts.push(format!("{} skipped", self.skipped));
        }
        parts.join(", ")
    }
}

/// Fetch current prices for multiple symbols from Yahoo Finance.
/// Returns a map of symbol -> price in dollars.
fn fetch_prices(symbols: &[String]) -> Result<HashMap<String, f64>> {
    use tokio::runtime::Runtime;
    use yahoo_finance_api as yahoo;

    if symbols.is_empty() {
        return Ok(HashMap::new());
    }

    let rt = Runtime::new().context("Failed to create tokio runtime")?;

    rt.block_on(async {
        let provider = yahoo::YahooConnector::new()?;
        let mut prices = HashMap::new();

        for symbol in symbols {
            match provider.get_latest_quotes(symbol, "1d").await {
                Ok(response) => {
                    if let Some(quote) = response.last_quote().ok() {
                        prices.insert(symbol.clone(), quote.close);
                    }
                }
                Err(e) => {
                    // Log but don't fail - some symbols may not be found
                    eprintln!("Warning: Could not fetch price for {}: {}", symbol, e);
                }
            }
        }

        Ok(prices)
    })
}

/// Update prices for all holdings with symbols in manual accounts.
pub fn update_prices(conn: &Connection) -> Result<PriceUpdateStats> {
    let now = Utc::now().timestamp();

    // Get all holdings with symbols from manual accounts
    let mut stmt = conn.prepare(
        "SELECT h.id, h.symbol, h.shares, h.account_id
         FROM holdings h
         JOIN accounts a ON h.account_id = a.id
         WHERE a.manual = 1 AND h.symbol IS NOT NULL AND h.symbol != ''"
    )?;

    let holdings: Vec<(String, String, String, String)> = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?
        .filter_map(|r| r.ok())
        .collect();

    if holdings.is_empty() {
        return Ok(PriceUpdateStats {
            updated: 0,
            failed: 0,
            skipped: 0,
            errors: vec!["No holdings with symbols found in manual accounts".to_string()],
        });
    }

    // Collect unique symbols
    let symbols: Vec<String> = holdings
        .iter()
        .map(|(_, sym, _, _)| sym.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    // Fetch prices
    let prices = fetch_prices(&symbols)?;

    let mut updated = 0;
    let mut failed = 0;
    let mut skipped = 0;
    let mut errors = Vec::new();
    let mut accounts_to_update = std::collections::HashSet::new();

    for (holding_id, symbol, shares_str, account_id) in &holdings {
        if let Some(&price) = prices.get(symbol) {
            let shares: f64 = shares_str.replace(',', "").parse().unwrap_or(0.0);
            let price_cents = (price * 100.0).round() as i64;
            let market_value_cents = (shares * price * 100.0).round() as i64;

            match conn.execute(
                "UPDATE holdings SET price = ?1, market_value = ?2, updated_at = ?3 WHERE id = ?4",
                rusqlite::params![price_cents, market_value_cents, now, holding_id],
            ) {
                Ok(_) => {
                    updated += 1;
                    accounts_to_update.insert(account_id.clone());
                }
                Err(e) => {
                    failed += 1;
                    errors.push(format!("{}: {}", symbol, e));
                }
            }
        } else {
            skipped += 1;
            errors.push(format!("{}: price not found", symbol));
        }
    }

    // Update account balances
    for account_id in accounts_to_update {
        conn.execute(
            "UPDATE accounts SET
                balance = (SELECT COALESCE(SUM(market_value), 0) FROM holdings WHERE account_id = ?1),
                balance_date = ?2,
                updated_at = ?2
             WHERE id = ?1",
            rusqlite::params![account_id, now],
        )?;
    }

    Ok(PriceUpdateStats {
        updated,
        failed,
        skipped,
        errors,
    })
}

/// Run price update and print results.
pub fn run() -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint init' first.")?;

    println!("Fetching latest prices from Yahoo Finance...");
    let stats = update_prices(&conn)?;

    println!("Price update: {}", stats.summary());

    if !stats.errors.is_empty() && stats.failed > 0 {
        eprintln!("Errors:");
        for err in &stats.errors {
            if !err.contains("price not found") {
                eprintln!("  - {}", err);
            }
        }
    }

    Ok(())
}

/// Run price update with an existing connection (for use during sync).
pub fn run_with_conn(conn: &Connection) -> Result<PriceUpdateStats> {
    update_prices(conn)
}
