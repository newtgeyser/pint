use anyhow::{Context, Result};
use rusqlite::Connection;
use serde::Deserialize;

use crate::db;

const DEFAULT_RULES: &str = include_str!("../../default_rules.toml");

#[derive(Debug, Deserialize)]
struct RulesConfig {
    #[serde(default)]
    rules: Vec<Rule>,
}

#[derive(Debug, Deserialize)]
struct Rule {
    pattern: String,
    category: String,
    #[serde(rename = "match", default)]
    match_mode: MatchModeConfig,
}

#[derive(Debug, Deserialize, Default, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum MatchModeConfig {
    #[default]
    Substring,
    Token,
}

impl From<MatchModeConfig> for db::MatchMode {
    fn from(value: MatchModeConfig) -> Self {
        match value {
            MatchModeConfig::Substring => db::MatchMode::Substring,
            MatchModeConfig::Token => db::MatchMode::Token,
        }
    }
}

/// List all rules from the database (CLI command).
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
        println!("No rules found. Run 'pint setup' to import default rules.");
        return Ok(());
    }

    println!("{:<30} {:<10} CATEGORY", "PATTERN", "MATCH");
    println!("{}", "-".repeat(70));

    let count = rules.len();
    for (pattern, match_mode, category) in rules {
        println!("{:<30} {:<10} {}", pattern, match_mode, category);
    }

    println!("\n{} rules total", count);

    Ok(())
}

/// Import default rules from embedded config into the database.
/// Returns (rules_imported, new_categories_created).
pub fn import_default_rules(conn: &Connection) -> Result<(usize, usize)> {
    use std::collections::HashSet;

    let config: RulesConfig =
        toml::from_str(DEFAULT_RULES).context("Failed to parse embedded default rules")?;

    // Get existing categories before import
    let existing: HashSet<String> = conn
        .prepare("SELECT name FROM categories")?
        .query_map([], |row| row.get(0))?
        .collect::<Result<HashSet<_>, _>>()?;

    let mut imported = 0;
    let mut new_categories: HashSet<String> = HashSet::new();

    for rule in &config.rules {
        let category_id = db::get_or_create_category(conn, &rule.category)?;

        if !existing.contains(&rule.category) {
            new_categories.insert(rule.category.clone());
        }

        db::upsert_merchant_rule_with_mode(
            conn,
            &rule.pattern,
            category_id,
            rule.match_mode.into(),
        )?;
        imported += 1;
    }

    Ok((imported, new_categories.len()))
}

/// Apply all rules to uncategorized transactions.
/// Returns the number of transactions categorized.
pub fn auto_categorize_all(conn: &Connection) -> Result<usize> {
    let mut categorized = 0;

    let rules = db::list_merchant_rules(conn)?;

    let tx_ids: Vec<(String, String)> = {
        let mut stmt =
            conn.prepare("SELECT id, description FROM transactions WHERE category_id IS NULL")?;

        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?
    };

    for (tx_id, description) in tx_ids {
        if let Some(category_id) =
            db::find_category_for_description_with_rules(&description, &rules)
        {
            db::categorize_transaction(conn, &tx_id, category_id)?;
            categorized += 1;
        }
    }

    Ok(categorized)
}
