use anyhow::{Context, Result, bail};
use chrono::Utc;
use std::cmp::Ordering;

use crate::db::{
    self,
    models::{Account, Holding},
};
use crate::util::truncate;

/// Update account balance to sum of holdings market values
fn update_account_balance(conn: &rusqlite::Connection, account_id: &str) -> Result<()> {
    let now = Utc::now().timestamp();
    conn.execute(
        "UPDATE accounts SET
            balance = (SELECT COALESCE(SUM(market_value), 0) FROM holdings WHERE account_id = ?1),
            balance_date = ?2,
            updated_at = ?2
         WHERE id = ?1",
        rusqlite::params![account_id, now],
    )?;
    Ok(())
}

#[derive(Clone, Copy)]
enum SortColumn {
    Symbol,
    Description,
    Shares,
    Price,
    Value,
    Gain,
    Loss,
}

fn parse_sort_column(s: &str) -> Result<SortColumn> {
    match s.to_lowercase().as_str() {
        "symbol" => Ok(SortColumn::Symbol),
        "description" | "desc" => Ok(SortColumn::Description),
        "shares" => Ok(SortColumn::Shares),
        "price" => Ok(SortColumn::Price),
        "value" => Ok(SortColumn::Value),
        "gain" | "gain/loss" => Ok(SortColumn::Gain),
        "loss" => Ok(SortColumn::Loss),
        other => bail!(
            "Unknown sort column '{}'. Valid: symbol, description, shares, price, value, gain, loss",
            other
        ),
    }
}

fn gain_pct(holding: &Holding) -> Option<f64> {
    match (holding.cost_basis, holding.market_value) {
        (Some(cost), Some(value)) if cost > 0 => {
            Some(((value - cost) as f64 / cost as f64) * 100.0)
        }
        _ => None,
    }
}

fn compare_holdings(a: &Holding, b: &Holding, sort_by: SortColumn) -> Ordering {
    match sort_by {
        SortColumn::Symbol => {
            let a_sym = a.symbol.as_deref().unwrap_or("");
            let b_sym = b.symbol.as_deref().unwrap_or("");
            a_sym.to_lowercase().cmp(&b_sym.to_lowercase())
        }
        SortColumn::Description => {
            let a_desc = a.description.as_deref().unwrap_or("");
            let b_desc = b.description.as_deref().unwrap_or("");
            a_desc.to_lowercase().cmp(&b_desc.to_lowercase())
        }
        SortColumn::Shares => {
            let a_shares: f64 = a.shares.replace(',', "").parse().unwrap_or(0.0);
            let b_shares: f64 = b.shares.replace(',', "").parse().unwrap_or(0.0);
            b_shares.partial_cmp(&a_shares).unwrap_or(Ordering::Equal)
        }
        SortColumn::Price => {
            let a_price = a.price.unwrap_or(0);
            let b_price = b.price.unwrap_or(0);
            b_price.cmp(&a_price)
        }
        SortColumn::Value => {
            let a_val = a.market_value.unwrap_or(0);
            let b_val = b.market_value.unwrap_or(0);
            b_val.cmp(&a_val)
        }
        SortColumn::Gain => {
            let a_gain = gain_pct(a).unwrap_or(f64::NEG_INFINITY);
            let b_gain = gain_pct(b).unwrap_or(f64::NEG_INFINITY);
            b_gain.partial_cmp(&a_gain).unwrap_or(Ordering::Equal)
        }
        SortColumn::Loss => {
            let a_gain = gain_pct(a).unwrap_or(f64::INFINITY);
            let b_gain = gain_pct(b).unwrap_or(f64::INFINITY);
            a_gain.partial_cmp(&b_gain).unwrap_or(Ordering::Equal)
        }
    }
}

pub fn run(account_filter: Option<&str>, sort: &str) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint init' first.")?;
    let sort_by = parse_sort_column(sort)?;

    let mut holdings = match account_filter {
        Some(filter) => Holding::find_by_account_filter(&conn, filter)?,
        None => Holding::find_all(&conn)?,
    };

    holdings.sort_by(|a, b| compare_holdings(a, b, sort_by));

    if holdings.is_empty() {
        println!("No holdings found. Run 'pint sync' to fetch holdings from brokerage accounts.");
        return Ok(());
    }

    println!(
        "{:<8} {:<20} {:>8} {:>9} {:>12} {:>10}",
        "SYMBOL", "DESCRIPTION", "SHARES", "PRICE", "VALUE", "GAIN/LOSS"
    );
    println!("{}", "-".repeat(72));

    let mut total_cost = 0i64;
    let mut total_value = 0i64;

    for holding in &holdings {
        let symbol = holding.symbol.as_deref().unwrap_or("-");
        let desc = truncate(holding.description.as_deref().unwrap_or("-"), 20);

        let price_str = holding
            .price_dollars()
            .map(|p| format!("{:>9.2}", p))
            .unwrap_or_else(|| "      N/A".to_string());

        let value_str = holding
            .market_value_dollars()
            .map(|v| format!("{:>12.2}", v))
            .unwrap_or_else(|| "         N/A".to_string());

        let gain_str = gain_pct(holding)
            .map(|g| format!("{:+.1}%", g))
            .unwrap_or_else(|| "N/A".to_string());

        println!(
            "{:<8} {:<20} {:>8} {} {} {:>10}",
            truncate(symbol, 8),
            desc,
            truncate(&holding.shares, 8),
            price_str,
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

    let total_gain_pct = if total_cost > 0 {
        ((total_value - total_cost) as f64 / total_cost as f64) * 100.0
    } else {
        0.0
    };

    println!("{}", "-".repeat(72));
    println!(
        "{:<8} {:<20} {:>8} {:>9} {:>12.2} {:>+10.1}%",
        "",
        "TOTAL",
        "",
        "",
        total_value as f64 / 100.0,
        total_gain_pct,
    );

    Ok(())
}

pub fn add(
    account_query: &str,
    symbol: &str,
    shares: &str,
    price: f64,
    cost: Option<f64>,
    description: Option<&str>,
) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint init' first.")?;

    let account = Account::find_by_query(&conn, account_query)?
        .ok_or_else(|| anyhow::anyhow!("No account found matching '{}'", account_query))?;
    let account_id = account.id.clone();
    let account_name = account.display_name().to_string();

    let now = Utc::now().timestamp();
    let price_cents = (price * 100.0).round() as i64;
    let shares_float: f64 = shares.parse().context("Invalid shares value")?;
    let market_value_cents = (shares_float * price * 100.0).round() as i64;
    let cost_cents = cost.map(|c| (c * 100.0).round() as i64);

    // Generate unique ID
    let id = format!("{}:{}", account_id, symbol.to_uppercase());

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
            id,
            account_id,
            symbol.to_uppercase(),
            description,
            shares,
            price_cents,
            cost_cents,
            market_value_cents,
            now,
        ],
    )?;

    update_account_balance(&conn, &account_id)?;

    println!(
        "Added {} {} to '{}' (value: ${:.2})",
        shares,
        symbol.to_uppercase(),
        account_name,
        market_value_cents as f64 / 100.0
    );

    Ok(())
}

pub fn update(
    holding_query: &str,
    shares: Option<&str>,
    price: Option<f64>,
    cost: Option<f64>,
) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint init' first.")?;

    let holding = Holding::find_by_query(&conn, holding_query)?
        .ok_or_else(|| anyhow::anyhow!("No holding found matching '{}'", holding_query))?;

    let now = Utc::now().timestamp();
    let new_shares = shares.unwrap_or(&holding.shares);

    conn.execute(
        "UPDATE holdings SET updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now, holding.id],
    )?;

    if let Some(s) = shares {
        conn.execute(
            "UPDATE holdings SET shares = ?1 WHERE id = ?2",
            rusqlite::params![s, holding.id],
        )?;
    }

    if let Some(p) = price {
        let price_cents = (p * 100.0).round() as i64;
        let shares_float: f64 = new_shares.parse().unwrap_or(0.0);
        let market_value_cents = (shares_float * p * 100.0).round() as i64;

        conn.execute(
            "UPDATE holdings SET price = ?1, market_value = ?2 WHERE id = ?3",
            rusqlite::params![price_cents, market_value_cents, holding.id],
        )?;
    }

    if let Some(c) = cost {
        let cost_cents = (c * 100.0).round() as i64;
        conn.execute(
            "UPDATE holdings SET cost_basis = ?1 WHERE id = ?2",
            rusqlite::params![cost_cents, holding.id],
        )?;
    }

    update_account_balance(&conn, &holding.account_id)?;

    println!(
        "Updated holding {}",
        holding.symbol.as_deref().unwrap_or(&holding.id)
    );

    Ok(())
}

pub fn remove(holding_query: &str) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint init' first.")?;

    let holding = Holding::find_by_query(&conn, holding_query)?
        .ok_or_else(|| anyhow::anyhow!("No holding found matching '{}'", holding_query))?;

    conn.execute("DELETE FROM holdings WHERE id = ?1", [&holding.id])?;

    update_account_balance(&conn, &holding.account_id)?;

    println!(
        "Removed holding {}",
        holding.symbol.as_deref().unwrap_or(&holding.id)
    );

    Ok(())
}
