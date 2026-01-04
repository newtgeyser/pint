use anyhow::{Context, Result};

use crate::db;

pub fn run() -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint init' first.")?;

    let mut stmt = conn.prepare(
        "SELECT r.pattern, r.match_mode, c.name as category
         FROM merchant_rules r
         JOIN categories c ON r.category_id = c.id
         ORDER BY c.name, r.match_mode, r.pattern",
    )?;

    let rules: Vec<(String, String, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<Result<Vec<_>, _>>()?;

    if rules.is_empty() {
        println!("No rules found. Run 'pint import-rules' first.");
        return Ok(());
    }

    println!("{:<30} {:<10} {}", "PATTERN", "MATCH", "CATEGORY");
    println!("{}", "-".repeat(70));

    let count = rules.len();
    for (pattern, match_mode, category) in rules {
        println!("{:<30} {:<10} {}", pattern, match_mode, category);
    }

    println!("\n{} rules total", count);

    Ok(())
}
