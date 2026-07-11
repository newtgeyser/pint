use anyhow::{Context, Result};

use crate::db::{
    self,
    insights::{DataHealth, NetWorthSnapshot},
};

fn format_amount(cents: i64) -> String {
    format!("${:.2}", cents as f64 / 100.0)
}

pub fn history(limit: usize, capture: bool) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint setup' first.")?;
    if capture {
        NetWorthSnapshot::capture(&conn)?;
    }
    let rows = NetWorthSnapshot::history(&conn, limit)?;
    if rows.is_empty() {
        println!("No net worth history yet. Run `pint history --capture` or sync first.");
        return Ok(());
    }
    println!(
        "{:<12} {:>16} {:>16} {:>16}",
        "DATE", "NET WORTH", "INVESTMENTS", "CHANGE"
    );
    let mut previous = None;
    for row in rows.iter().rev() {
        let change = previous.map(|value| row.net_worth - value);
        println!(
            "{:<12} {:>16} {:>16} {:>16}",
            row.snapshot_date,
            format_amount(row.net_worth),
            format_amount(row.brokerage + row.retirement),
            change.map(format_amount).unwrap_or_else(|| "-".to_string()),
        );
        previous = Some(row.net_worth);
    }
    Ok(())
}

pub fn health() -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint setup' first.")?;
    let health = DataHealth::load(&conn)?;
    println!("Data health: {} issue(s)", health.issue_count());
    println!(
        "  Stale account balances (>7 days): {}",
        health.stale_accounts
    );
    println!(
        "  Holdings without prices: {}",
        health.holdings_without_price
    );
    println!(
        "  Holdings without cost basis: {}",
        health.holdings_without_cost_basis
    );
    println!("  Assets without values: {}", health.assets_without_value);
    println!(
        "  Uncategorized transactions: {}",
        health.uncategorized_transactions
    );
    println!(
        "  Pending reimbursements over 30 days: {}",
        health.pending_reimbursements_over_30_days
    );
    Ok(())
}
