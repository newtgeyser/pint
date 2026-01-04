use anyhow::{Context, Result};

use crate::db::{self, models::Holding};

pub fn run(account_filter: Option<&str>) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint init' first.")?;

    let (query, params): (&str, Vec<Box<dyn rusqlite::ToSql>>) = match account_filter {
        Some(filter) => (
            "SELECT h.id, h.account_id, h.symbol, h.description, h.shares, h.cost_basis, h.market_value, h.currency, h.created_at, h.updated_at
             FROM holdings h
             JOIN accounts a ON h.account_id = a.id
             WHERE a.id = ?1 OR a.id LIKE ?1 || '%' OR a.name LIKE '%' || ?1 || '%'
             ORDER BY h.market_value DESC NULLS LAST",
            vec![Box::new(filter.to_string())],
        ),
        None => (
            "SELECT id, account_id, symbol, description, shares, cost_basis, market_value, currency, created_at, updated_at
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
        "{:<8} {:<20} {:>10} {:>12} {:>12} {:>10}",
        "SYMBOL", "DESCRIPTION", "SHARES", "COST", "VALUE", "GAIN/LOSS"
    );
    println!("{}", "-".repeat(76));

    let mut total_cost = 0i64;
    let mut total_value = 0i64;

    for holding in &holdings {
        let symbol = holding.symbol.as_deref().unwrap_or("-");
        let desc = truncate(holding.description.as_deref().unwrap_or("-"), 20);

        let cost = holding.cost_basis.unwrap_or(0);
        let value = holding.market_value.unwrap_or(0);
        let gain = value - cost;
        let gain_pct = if cost > 0 {
            (gain as f64 / cost as f64) * 100.0
        } else {
            0.0
        };

        let cost_str = if holding.cost_basis.is_some() {
            format!("{:>12.2}", cost as f64 / 100.0)
        } else {
            "         N/A".to_string()
        };

        let value_str = if holding.market_value.is_some() {
            format!("{:>12.2}", value as f64 / 100.0)
        } else {
            "         N/A".to_string()
        };

        let gain_str = if holding.cost_basis.is_some() && holding.market_value.is_some() {
            format!("{:+.1}%", gain_pct)
        } else {
            "N/A".to_string()
        };

        println!(
            "{:<8} {:<20} {:>10} {} {} {:>10}",
            truncate(symbol, 8),
            desc,
            truncate(&holding.shares, 10),
            cost_str,
            value_str,
            gain_str,
        );

        if let Some(c) = holding.cost_basis {
            total_cost += c;
        }
        if let Some(v) = holding.market_value {
            total_value += v;
        }
    }

    let total_gain = total_value - total_cost;
    let total_gain_pct = if total_cost > 0 {
        (total_gain as f64 / total_cost as f64) * 100.0
    } else {
        0.0
    };

    println!("{}", "-".repeat(76));
    println!(
        "{:<8} {:<20} {:>10} {:>12.2} {:>12.2} {:>+10.1}%",
        "",
        "TOTAL",
        "",
        total_cost as f64 / 100.0,
        total_value as f64 / 100.0,
        total_gain_pct,
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
