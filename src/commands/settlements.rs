use anyhow::{Context, Result, bail};
use chrono::{NaiveDate, Utc};

use crate::db::{
    self,
    models::Reimburser,
    reimbursements::{self, AllocationRequest, ReimbursementPayment},
};

pub fn aging() -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint setup' first.")?;
    let rows = reimbursements::aging_summary(&conn, Utc::now().timestamp())?;
    if rows.is_empty() {
        println!("No outstanding reimbursements.");
        return Ok(());
    }
    println!(
        "{:<20} {:>12} {:>12} {:>12} {:>12} {:>12}",
        "ENTITY", "0-30", "31-60", "61-90", "90+", "TOTAL"
    );
    for row in rows {
        println!(
            "{:<20} {:>12} {:>12} {:>12} {:>12} {:>12}",
            row.reimburser,
            money(row.current),
            money(row.days_31_60),
            money(row.days_61_90),
            money(row.over_90_days),
            money(row.total),
        );
    }
    Ok(())
}

pub fn record_payment(
    entity: &str,
    amount: f64,
    date: Option<&str>,
    source_transaction: Option<&str>,
    reference: Option<&str>,
    note: Option<&str>,
    allocations: &[String],
) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint setup' first.")?;
    let reimburser = Reimburser::find_by_name(&conn, entity)?
        .with_context(|| format!("Reimburser '{}' not found", entity))?;
    let amount = dollars_to_cents(amount)?;
    let received_at = date
        .map(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d"))
        .transpose()
        .context("Payment date must use YYYY-MM-DD")?
        .map(|value| value.and_hms_opt(12, 0, 0).unwrap().and_utc().timestamp())
        .unwrap_or_else(|| Utc::now().timestamp());

    let parsed = allocations
        .iter()
        .map(|allocation| {
            let (transaction_id, value) = allocation
                .split_once('=')
                .with_context(|| format!("Allocation '{}' must use TX_ID=AMOUNT", allocation))?;
            let value: f64 = value
                .parse()
                .with_context(|| format!("Invalid allocation amount in '{}'", allocation))?;
            Ok((transaction_id, dollars_to_cents(value)?))
        })
        .collect::<Result<Vec<_>>>()?;
    let requests = parsed
        .iter()
        .map(|(transaction_id, amount)| AllocationRequest {
            transaction_id,
            amount: *amount,
        })
        .collect::<Vec<_>>();

    let payment = ReimbursementPayment::create(
        &conn,
        reimburser.id,
        amount,
        received_at,
        source_transaction,
        reference,
        note,
        &requests,
    )?;
    let unallocated = ReimbursementPayment::unallocated_amount(&conn, payment.id)?;
    println!(
        "Recorded payment {} for {}.",
        money(amount),
        reimburser.name
    );
    if unallocated > 0 {
        println!("Unallocated balance: {}", money(unallocated));
    }
    Ok(())
}

fn dollars_to_cents(value: f64) -> Result<i64> {
    if !value.is_finite() || value <= 0.0 {
        bail!("Amount must be a positive number");
    }
    Ok((value * 100.0).round() as i64)
}

fn money(cents: i64) -> String {
    format!("${:.2}", cents as f64 / 100.0)
}
