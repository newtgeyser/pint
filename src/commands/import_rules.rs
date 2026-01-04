use anyhow::{Context, Result};

use crate::{db, rules};

pub fn run() -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint init' first.")?;

    let rules_path = rules::rules_path()?;
    println!("Importing rules from {}", rules_path.display());

    let (rules_imported, categories_created) = rules::import_rules(&conn)?;

    println!(
        "Imported {} rules, created {} new categories",
        rules_imported, categories_created
    );

    Ok(())
}
