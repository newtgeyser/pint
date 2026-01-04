use anyhow::{Context, Result};

use crate::db::{self, models::Category};

pub fn run() -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint init' first.")?;

    let mut stmt = conn.prepare(
        "SELECT id, name, parent_id, created_at
         FROM categories
         ORDER BY name",
    )?;

    let categories: Vec<Category> = stmt
        .query_map([], |row| Category::from_row(row))?
        .collect::<Result<Vec<_>, _>>()?;

    if categories.is_empty() {
        println!("No categories found. Run 'pint import-rules' first.");
        return Ok(());
    }

    println!("{:<6} {}", "ID", "CATEGORY");
    println!("{}", "-".repeat(40));

    for cat in categories {
        println!("{:<6} {}", cat.id, cat.name);
    }

    Ok(())
}
