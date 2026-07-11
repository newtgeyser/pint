use anyhow::{Context, Result};
use chrono::{DateTime, Utc};

use crate::db::{self, uncategorized};

pub fn list(limit: Option<usize>) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint init' first.")?;
    let groups = uncategorized::list_groups(&conn, limit)?;

    if groups.is_empty() {
        println!("No uncategorized transactions.");
        return Ok(());
    }

    println!(
        "{:<32} {:>5} {:>13} {:<10} EXAMPLE",
        "MERCHANT", "COUNT", "NET", "NEWEST"
    );
    println!("{}", "-".repeat(90));
    for group in groups {
        let newest = DateTime::<Utc>::from_timestamp(group.newest_posted, 0)
            .map(|date| date.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "-".to_string());
        println!(
            "{:<32} {:>5} {:>13} {:<10} {}",
            truncate(&group.merchant, 32),
            group.count,
            format_amount(group.net_amount),
            newest,
            truncate(&group.example, 40),
        );
    }
    Ok(())
}

pub fn categorize(
    merchant: &str,
    category: &str,
    create_rule: bool,
    pattern: Option<&str>,
) -> Result<()> {
    let mut conn = db::open().context("Database not found. Run 'pint init' first.")?;
    let rule_pattern = if create_rule {
        Some(pattern.unwrap_or(merchant))
    } else {
        None
    };
    let result = uncategorized::categorize_group(&mut conn, merchant, category, rule_pattern)?;

    println!(
        "Categorized {} transaction{} as '{}'.",
        result.categorized,
        if result.categorized == 1 { "" } else { "s" },
        category.trim(),
    );
    if let Some(pattern) = result.rule_pattern {
        println!("Created merchant rule '{}'.", pattern);
    }
    Ok(())
}

fn format_amount(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let absolute = cents.unsigned_abs();
    format!("{}${}.{:02}", sign, absolute / 100, absolute % 100)
}

fn truncate(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_string();
    }
    value
        .chars()
        .take(width.saturating_sub(3))
        .collect::<String>()
        + "..."
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_signed_amounts() {
        assert_eq!(format_amount(-12345), "-$123.45");
        assert_eq!(format_amount(99), "$0.99");
    }

    #[test]
    fn truncates_on_character_boundaries() {
        assert_eq!(truncate("Coffee Shop", 8), "Coffe...");
        assert_eq!(truncate("Cafe", 8), "Cafe");
    }
}
