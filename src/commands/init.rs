use anyhow::Result;

use crate::{config, db, rules};

pub fn run() -> Result<()> {
    let db_path = config::db_path()?;
    let is_new = !db_path.exists();

    let conn = db::init()?;

    if is_new {
        println!("Initialized database at {}", db_path.display());
    }

    // Import default rules if rules table is empty
    let rule_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM merchant_rules",
        [],
        |row| row.get(0),
    )?;

    if rule_count == 0 {
        let (rules_imported, categories_created) = rules::import_rules(&conn)?;
        println!(
            "Imported {} rules, created {} categories",
            rules_imported, categories_created
        );
    }

    Ok(())
}
