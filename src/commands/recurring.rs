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
        println!("  - Consistent intervals (monthly: 25-35 days, bi-monthly: 55-70 days)");
        return Ok(());
    }

    println!(
        "{:<25} {:>12} {:>10} {:>6} {:>12}",
        "MERCHANT", "AMOUNT", "FREQUENCY", "COUNT", "LAST"
    );
    println!("{}", "-".repeat(70));

    for pattern in &patterns {
        let category_str = pattern
            .category
            .as_ref()
            .map(|c| format!(" [{}]", c))
            .unwrap_or_default();

        let merchant_display = if pattern.merchant.len() > 24 {
            format!("{}...", &pattern.merchant[..21])
        } else {
            pattern.merchant.clone()
        };

        println!(
            "{:<25} {:>12.2} {:>10} {:>6} {:>12}{}",
            merchant_display,
            pattern.avg_amount,
            pattern.frequency,
            pattern.occurrences,
            pattern.last_date,
            category_str,
        );
    }

    println!("\n{} recurring patterns detected", patterns.len());

    Ok(())
}
