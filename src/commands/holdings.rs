use anyhow::{Context, Result};

use crate::db::{self, models::Holding};

pub fn run(account_filter: Option<&str>) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint init' first.")?;

    let (query, params): (&str, Vec<Box<dyn rusqlite::ToSql>>) = match account_filter {
        Some(filter) => (
            "SELECT h.id, h.account_id, h.symbol, h.description, h.shares, h.price, h.market_value, h.currency, h.created_at, h.updated_at
             FROM holdings h
             JOIN accounts a ON h.account_id = a.id
             WHERE a.id = ?1 OR a.id LIKE ?1 || '%' OR a.name LIKE '%' || ?1 || '%'
             ORDER BY h.market_value DESC NULLS LAST",
            vec![Box::new(filter.to_string())],
        ),
        None => (
            "SELECT id, account_id, symbol, description, shares, price, market_value, currency, created_at, updated_at
             FROM holdings
             ORDER BY market_value DESC NULLS LAST",
            vec![],
        ),
    };

    let mut stmt = conn.prepare(query)?;
    let holdings: Vec<Holding> = stmt
        .query_map(rusqlite::params_from_iter(params.iter()), |row| {
            Holding::from_row(row)
        })?
        .collect::<Result<Vec<_>, _>>()?;

    if holdings.is_empty() {
        println!("No holdings found. Run 'pint sync' to fetch holdings from brokerage accounts.");
        return Ok(());
    }

    println!(
        "{:<8} {:<24} {:>10} {:>10} {:>14}",
        "SYMBOL", "DESCRIPTION", "SHARES", "PRICE", "VALUE"
    );
    println!("{}", "-".repeat(70));

    let mut total_value = 0i64;

    for holding in &holdings {
        let symbol = holding.symbol.as_deref().unwrap_or("-");
        let desc = truncate(holding.description.as_deref().unwrap_or("-"), 24);

        let price_str = holding
            .price_dollars()
            .map(|p| format!("{:>10.2}", p))
            .unwrap_or_else(|| "       N/A".to_string());

        let value_str = holding
            .market_value_dollars()
            .map(|v| format!("{:>14.2}", v))
            .unwrap_or_else(|| "           N/A".to_string());

        println!(
            "{:<8} {:<24} {:>10} {} {}",
            truncate(symbol, 8),
            desc,
            truncate(&holding.shares, 10),
            price_str,
            value_str,
        );

        if let Some(v) = holding.market_value {
            total_value += v;
        }
    }

    println!("{}", "-".repeat(70));
    println!(
        "{:<8} {:<24} {:>10} {:>10} {:>14.2}",
        "",
        "TOTAL",
        "",
        "",
        total_value as f64 / 100.0,
    );

    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }

    let count = s.chars().count();
    if count <= max {
        return s.to_string();
    }
    if max <= 3 {
        return ".".repeat(max);
    }

    let mut out: String = s.chars().take(max - 3).collect();
    out.push_str("...");
    out
}
