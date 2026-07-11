pub mod insights;
pub mod models;
pub mod reimbursements;
pub mod schema;
pub mod uncategorized;

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension};
use std::path::Path;

use crate::config;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchMode {
    Substring,
    Token,
}

impl MatchMode {
    pub fn as_str(self) -> &'static str {
        match self {
            MatchMode::Substring => "substring",
            MatchMode::Token => "token",
        }
    }
}

impl std::str::FromStr for MatchMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "substring" => Ok(MatchMode::Substring),
            "token" => Ok(MatchMode::Token),
            other => Err(anyhow::anyhow!("unknown match mode: {}", other)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MerchantRuleEntry {
    pub pattern: String,
    pub category_id: i64,
    pub match_mode: MatchMode,
}

pub fn open() -> Result<Connection> {
    let db_path = config::db_path()?;
    if !db_path.exists() {
        anyhow::bail!("Database not found at {}", db_path.display());
    }
    open_at(&db_path)
}

pub fn open_at(path: &Path) -> Result<Connection> {
    if !path.exists() {
        anyhow::bail!("Database not found at {}", path.display());
    }

    let conn = Connection::open(path)
        .with_context(|| format!("Failed to open database at {}", path.display()))?;

    conn.execute_batch("PRAGMA foreign_keys = ON;")?;

    if !schema::is_initialized(&conn)? {
        anyhow::bail!(
            "Database at {} is not initialized. Run 'pint init' first.",
            path.display()
        );
    }

    // Run migrations to ensure schema is up to date.
    schema::migrate(&conn)?;

    Ok(conn)
}

fn open_or_create_at(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)
        .with_context(|| format!("Failed to open database at {}", path.display()))?;

    conn.execute_batch("PRAGMA foreign_keys = ON;")?;

    schema::create_tables(&conn)?;
    schema::migrate(&conn)?;

    Ok(conn)
}

pub fn init() -> Result<Connection> {
    config::ensure_data_dir()?;
    let db_path = config::db_path()?;
    open_or_create_at(&db_path)
}

pub fn get_config(conn: &Connection, key: &str) -> Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT value FROM config WHERE key = ?1")?;
    let result = stmt.query_row([key], |row| row.get(0)).optional()?;
    Ok(result)
}

pub fn set_config(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO config (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [key, value],
    )?;
    Ok(())
}

pub fn get_or_create_category(conn: &Connection, name: &str) -> Result<i64> {
    let now = Utc::now().timestamp();

    // Try to find existing category
    let existing: Option<i64> = conn
        .prepare("SELECT id FROM categories WHERE name = ?1")?
        .query_row([name], |row| row.get(0))
        .optional()?;

    if let Some(id) = existing {
        return Ok(id);
    }

    // Create new category
    conn.execute(
        "INSERT INTO categories (name, created_at) VALUES (?1, ?2)",
        rusqlite::params![name, now],
    )?;

    Ok(conn.last_insert_rowid())
}

pub fn upsert_merchant_rule_with_mode(
    conn: &Connection,
    pattern: &str,
    category_id: i64,
    match_mode: MatchMode,
) -> Result<()> {
    let now = Utc::now().timestamp();
    let normalized = pattern.trim().to_uppercase();

    conn.execute(
        "INSERT INTO merchant_rules (pattern, match_mode, category_id, created_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(pattern) DO UPDATE SET
            match_mode = excluded.match_mode,
            category_id = excluded.category_id",
        rusqlite::params![normalized, match_mode.as_str(), category_id, now],
    )?;

    Ok(())
}

pub fn list_merchant_rules(conn: &Connection) -> Result<Vec<MerchantRuleEntry>> {
    let mut stmt = conn.prepare(
        "SELECT pattern, category_id, match_mode
         FROM merchant_rules
         ORDER BY LENGTH(pattern) DESC",
    )?;

    let rules = stmt
        .query_map([], |row| {
            let match_mode: String = row.get(2)?;
            let match_mode = match match_mode.as_str() {
                "substring" => MatchMode::Substring,
                "token" => MatchMode::Token,
                other => {
                    #[derive(Debug)]
                    struct InvalidMatchMode(String);
                    impl std::fmt::Display for InvalidMatchMode {
                        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                            write!(f, "unknown match_mode value: {}", self.0)
                        }
                    }
                    impl std::error::Error for InvalidMatchMode {}

                    return Err(rusqlite::Error::FromSqlConversionFailure(
                        2,
                        rusqlite::types::Type::Text,
                        Box::new(InvalidMatchMode(other.to_string())),
                    ));
                }
            };
            Ok(MerchantRuleEntry {
                pattern: row.get(0)?,
                category_id: row.get(1)?,
                match_mode,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(rules)
}

pub fn find_category_for_description_with_rules(
    description: &str,
    rules: &[MerchantRuleEntry],
) -> Option<i64> {
    let desc_upper = description.to_uppercase();

    for rule in rules {
        if merchant_rule_matches(&desc_upper, &rule.pattern, rule.match_mode) {
            return Some(rule.category_id);
        }
    }
    None
}

fn merchant_rule_matches(
    description_upper: &str,
    pattern_upper: &str,
    match_mode: MatchMode,
) -> bool {
    match match_mode {
        MatchMode::Substring => description_upper.contains(pattern_upper),
        MatchMode::Token => is_token_match(description_upper, pattern_upper),
    }
}

/// Check if pattern matches as a token (word boundary on both sides) in the text.
/// Both arguments should be in the same case (uppercase recommended).
pub fn is_token_match(text: &str, pattern: &str) -> bool {
    text.match_indices(pattern).any(|(idx, _)| {
        let before = text[..idx].chars().last();
        let after = text[idx + pattern.len()..].chars().next();
        let ok_before = before.is_none_or(|c| !c.is_ascii_alphanumeric());
        let ok_after = after.is_none_or(|c| !c.is_ascii_alphanumeric());
        ok_before && ok_after
    })
}

pub fn categorize_transaction(conn: &Connection, tx_id: &str, category_id: i64) -> Result<bool> {
    let now = Utc::now().timestamp();

    let rows = conn.execute(
        "UPDATE transactions SET category_id = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![category_id, now, tx_id],
    )?;

    Ok(rows > 0)
}

/// Create an in-memory database for testing with schema initialized.
/// This is public so integration tests can use it.
pub fn open_in_memory() -> Result<Connection> {
    let conn = Connection::open_in_memory().context("Failed to create in-memory database")?;

    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    schema::create_tables(&conn)?;
    schema::migrate(&conn)?;

    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_match_avoids_substrings_inside_words() {
        let s = "FIRST TECH FCU";
        assert!(merchant_rule_matches(s, "IRS", MatchMode::Substring));
        assert!(!merchant_rule_matches(s, "IRS", MatchMode::Token));

        let s = "IRS TREAS 310 TAX REF";
        assert!(merchant_rule_matches(s, "IRS", MatchMode::Token));
    }

    #[test]
    fn token_match_requires_end_boundary() {
        // "MOBIL" should NOT match "MOBILE" as a token
        let s = "CAPITAL ONE MOBILE PMT";
        assert!(merchant_rule_matches(s, "MOBIL", MatchMode::Substring));
        assert!(!merchant_rule_matches(s, "MOBIL", MatchMode::Token));

        // But should match "MOBIL" exactly
        let s = "EXXON MOBIL PAYMENT";
        assert!(merchant_rule_matches(s, "MOBIL", MatchMode::Token));
    }
}
