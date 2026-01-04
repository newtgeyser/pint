use anyhow::{Context, Result};
use rusqlite::Connection;
use serde::Deserialize;
use std::path::PathBuf;

use crate::{config, db};

const DEFAULT_RULES: &str = include_str!("../default_rules.toml");

#[derive(Debug, Deserialize)]
pub struct RulesConfig {
    #[serde(default)]
    pub rules: Vec<Rule>,
}

#[derive(Debug, Deserialize)]
pub struct Rule {
    pub pattern: String,
    pub category: String,
    #[serde(rename = "match", default)]
    pub match_mode: MatchModeConfig,
}

#[derive(Debug, Deserialize, Default, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum MatchModeConfig {
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

pub fn rules_path() -> Result<PathBuf> {
    let mut path = config::data_dir()?;
    path.push("rules.toml");
    Ok(path)
}

pub fn ensure_rules_file() -> Result<PathBuf> {
    let path = rules_path()?;

    if !path.exists() {
        config::ensure_data_dir()?;
        std::fs::write(&path, DEFAULT_RULES)
            .with_context(|| format!("Failed to write default rules to {}", path.display()))?;
    }

    Ok(path)
}

pub fn load_rules() -> Result<RulesConfig> {
    let path = ensure_rules_file()?;
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read rules from {}", path.display()))?;

    let config: RulesConfig =
        toml::from_str(&content).with_context(|| format!("Failed to parse {}", path.display()))?;

    Ok(config)
}

pub fn import_rules(conn: &Connection) -> Result<(usize, usize)> {
    use std::collections::HashSet;

    let config = load_rules()?;

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

pub fn auto_categorize_all(conn: &Connection) -> Result<usize> {
    let mut categorized = 0;

    let rules = db::list_merchant_rules(conn)?;

    let tx_ids: Vec<(String, String)> = {
        let mut stmt = conn.prepare(
            "SELECT id, description FROM transactions WHERE category_id IS NULL",
        )?;

        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?
    };

    for (tx_id, description) in tx_ids {
        if let Some(category_id) = db::find_category_for_description_with_rules(&description, &rules)
        {
            db::categorize_transaction(conn, &tx_id, category_id)?;
            categorized += 1;
        }
    }

    Ok(categorized)
}
