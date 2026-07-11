use anyhow::{Context, Result, bail};
use chrono::{TimeZone, Utc};
use rusqlite::OptionalExtension;

use crate::db::{
    self,
    models::{ReimbursableFilter, Reimburser, Transaction, TransactionRow},
};

fn reimbursement_balance_effect(amount: f64) -> f64 {
    -amount
}

/// Mark a transaction as reimbursable by an entity.
pub fn set(tx_id: &str, entity: &str) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint setup' first.")?;
    let reimburser = Reimburser::find_by_name(&conn, entity)?.ok_or_else(|| {
        anyhow::anyhow!(
            "Reimburser '{}' not found. Add it with 'pint reimbursers add'.",
            entity
        )
    })?;

    if !Transaction::set_reimburser(&conn, tx_id, reimburser.id)? {
        bail!("Transaction '{}' not found", tx_id);
    }

    println!("Marked {} as reimbursable by {}", tx_id, reimburser.name);
    Ok(())
}

/// Mark a reimbursable transaction as paid (reimbursed).
pub fn mark_paid(tx_id: &str) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint setup' first.")?;
    let existing: Option<Option<i64>> = conn
        .query_row(
            "SELECT reimburser_id FROM transactions WHERE id = ?1",
            [tx_id],
            |row| row.get(0),
        )
        .optional()?;

    match existing {
        None => bail!("Transaction '{}' not found", tx_id),
        Some(None) => bail!(
            "Transaction '{}' has no reimburser. Run 'pint reimburse <tx-id> <entity>' first.",
            tx_id
        ),
        Some(Some(_)) => {}
    }

    let now = Utc::now().timestamp();
    Transaction::set_reimbursed_at(&conn, tx_id, Some(now))?;
    println!("Marked {} as reimbursed", tx_id);
    Ok(())
}

/// Clear all reimbursement state on a transaction.
pub fn clear(tx_id: &str) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint setup' first.")?;
    if !Transaction::clear_reimburser(&conn, tx_id)? {
        bail!("Transaction '{}' not found", tx_id);
    }
    println!("Cleared reimbursement state for {}", tx_id);
    Ok(())
}

/// List all reimbursable transactions, optionally filtered by status and/or entity.
pub fn list(filter: ReimbursableFilter, entity: Option<&str>) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint setup' first.")?;
    let rows = TransactionRow::find_reimbursable(&conn, filter, entity)?;

    if rows.is_empty() {
        println!("No reimbursable transactions found.");
        return Ok(());
    }

    println!(
        "{:<12} {:<20} {:<32} {:>10}  {:<18} {}",
        "DATE", "REIMBURSER", "DESCRIPTION", "AMOUNT", "STATUS", "TX ID"
    );
    println!("{}", "-".repeat(110));

    let mut total_pending: f64 = 0.0;
    let mut total_paid: f64 = 0.0;

    for r in &rows {
        let entity = r.reimburser.as_deref().unwrap_or("");
        let status = match r.reimbursed_at {
            Some(ts) => {
                let date = Utc
                    .timestamp_opt(ts, 0)
                    .single()
                    .map(|d| d.format("%Y-%m-%d").to_string())
                    .unwrap_or_else(|| "paid".to_string());
                total_paid += reimbursement_balance_effect(r.amount);
                format!("paid {}", date)
            }
            None => {
                total_pending += reimbursement_balance_effect(r.amount);
                "pending".to_string()
            }
        };

        let desc: String = r.description.chars().take(30).collect();
        println!(
            "{:<12} {:<20} {:<32} {:>10.2}  {:<18} {}",
            r.date,
            entity.chars().take(20).collect::<String>(),
            desc,
            r.amount,
            status,
            r.id,
        );
    }

    println!("{}", "-".repeat(110));
    println!(
        "  Pending: ${:.2}    Paid: ${:.2}    Total: ${:.2}",
        total_pending,
        total_paid,
        total_pending + total_paid
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::reimbursement_balance_effect;

    #[test]
    fn expenses_increase_and_credits_reduce_reimbursement_balance() {
        let expense = reimbursement_balance_effect(-100.0);
        let refund = reimbursement_balance_effect(25.0);

        assert_eq!(expense + refund, 75.0);
    }
}
