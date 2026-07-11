use anyhow::{Context, Result};

use crate::db;
use crate::db::models::RecurringPattern;

pub fn run() -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint setup' first.")?;

    let patterns = RecurringPattern::detect(&conn)?;

    if patterns.is_empty() {
        println!("No recurring transactions detected.");
        println!("\nRecurring transactions are identified by:");
        println!("  - At least 3 similar transactions");
        println!(
            "  - Consistent weekly, biweekly, monthly, bi-monthly, quarterly, or annual intervals"
        );
        return Ok(());
    }

    println!(
        "{:<22} {:>10} {:>10} {:>9} {:>10} {:>12} {:>10}",
        "MERCHANT", "AMOUNT", "FREQUENCY", "MONTHLY", "VARIANCE", "NEXT", "STATUS"
    );
    println!("{}", "-".repeat(91));

    for pattern in &patterns {
        let category_str = pattern
            .category
            .as_ref()
            .map(|c| format!(" [{}]", c))
            .unwrap_or_default();

        let merchant_display = if pattern.merchant.len() > 21 {
            format!("{}...", &pattern.merchant[..18])
        } else {
            pattern.merchant.clone()
        };

        println!(
            "{:<22} {:>10.2} {:>10} {:>9.2} {:>10.2} {:>12} {:>10}{}",
            merchant_display,
            pattern.avg_amount,
            pattern.frequency,
            pattern.monthly_commitment,
            pattern.amount_variance,
            pattern.expected_next_date,
            pattern.status,
            category_str,
        );
    }

    let monthly_total: f64 = patterns
        .iter()
        .filter(|pattern| pattern.status != "inactive")
        .map(|pattern| pattern.monthly_commitment)
        .sum();
    println!(
        "\n{} recurring patterns detected; ${monthly_total:.2} active monthly commitment",
        patterns.len()
    );

    Ok(())
}
