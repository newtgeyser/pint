use anyhow::{Context, Result, bail};

use super::rules;
use crate::db;

pub fn run_auto() -> Result<()> {
    run_auto_quiet(false)
}

pub fn run_auto_quiet(quiet: bool) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint init' first.")?;

    // Check if there are any rules in the database
    let rule_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM merchant_rules", [], |row| row.get(0))?;

    if rule_count == 0 {
        bail!("No rules found. Run 'pint setup' to import default rules.");
    }

    let categorized = rules::auto_categorize_all(&conn)?;

    if !quiet {
        if categorized == 0 {
            println!("No uncategorized transactions found (or no matching rules).");
        } else {
            println!("Categorized {} transactions.", categorized);
        }
    }

    Ok(())
}

pub fn run_manual(tx_id: &str, category_name: &str) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint init' first.")?;

    // Get or create the category
    let category_id = db::get_or_create_category(&conn, category_name)?;

    // Update the transaction
    if db::categorize_transaction(&conn, tx_id, category_id)? {
        println!("Transaction {} categorized as '{}'.", tx_id, category_name);
    } else {
        bail!("Transaction '{}' not found.", tx_id);
    }

    Ok(())
}

pub fn run_learn(tx_id: &str, category_name: &str, pattern: &str, match_mode: &str) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint init' first.")?;

    // Get or create the category
    let category_id = db::get_or_create_category(&conn, category_name)?;

    // Categorize the transaction
    if !db::categorize_transaction(&conn, tx_id, category_id)? {
        bail!("Transaction '{}' not found.", tx_id);
    }

    // Add the rule
    let match_mode: db::MatchMode = match_mode.parse()?;
    db::upsert_merchant_rule_with_mode(&conn, pattern, category_id, match_mode)?;

    println!(
        "Transaction {} categorized as '{}' and rule '{}' ({}) added.",
        tx_id,
        category_name,
        pattern.trim().to_uppercase(),
        match_mode.as_str()
    );

    Ok(())
}
