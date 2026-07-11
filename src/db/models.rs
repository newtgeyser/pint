use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, Row};

#[derive(Debug, Clone)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub nickname: Option<String>,
    pub institution: Option<String>,
    pub account_type: String,
    pub balance: Option<i64>,
    pub balance_date: Option<i64>,
    pub currency: String,
    pub manual: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Account {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            name: row.get("name")?,
            nickname: row.get("nickname")?,
            institution: row.get("institution")?,
            account_type: row.get("account_type")?,
            balance: row.get("balance")?,
            balance_date: row.get("balance_date")?,
            currency: row.get("currency")?,
            manual: row.get::<_, i64>("manual")? != 0,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }

    pub fn balance_dollars(&self) -> Option<f64> {
        self.balance.map(|cents| cents as f64 / 100.0)
    }

    /// Returns the display name: nickname if set, otherwise name
    pub fn display_name(&self) -> &str {
        self.nickname.as_deref().unwrap_or(&self.name)
    }

    /// Find an account by ID (exact or prefix), nickname, or name (partial match).
    pub fn find_by_query(conn: &Connection, query: &str) -> Result<Option<Account>> {
        let account = conn
            .query_row(
                "SELECT id, name, nickname, institution, account_type, balance, balance_date, currency, manual, created_at, updated_at
                 FROM accounts
                 WHERE id = ?1 OR id LIKE ?1 || '%' OR nickname LIKE '%' || ?1 || '%' OR name LIKE '%' || ?1 || '%'
                 LIMIT 1",
                [query],
                Account::from_row,
            )
            .optional()?;
        Ok(account)
    }

    /// Find all accounts, ordered by manual flag, type, then display name.
    pub fn find_all(conn: &Connection) -> Result<Vec<Account>> {
        let mut stmt = conn.prepare(
            "SELECT id, name, nickname, institution, account_type, balance, balance_date, currency, manual, created_at, updated_at
             FROM accounts
             ORDER BY manual DESC, account_type, COALESCE(nickname, name)"
        )?;
        let accounts = stmt
            .query_map([], Account::from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(accounts)
    }

    /// Find all accounts as (id, display_name) tuples for selection dialogs.
    pub fn find_all_for_select(conn: &Connection) -> Result<Vec<(String, String)>> {
        let mut stmt = conn.prepare(
            "SELECT id, COALESCE(nickname, name) FROM accounts
             ORDER BY manual DESC, account_type, COALESCE(nickname, name)",
        )?;
        let items = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(items)
    }
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub id: String,
    pub account_id: String,
    pub posted: i64,
    pub amount: i64,
    pub description: String,
    pub pending: bool,
    pub category_id: Option<i64>,
    pub reimburser_id: Option<i64>,
    pub reimbursed_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Transaction {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            account_id: row.get("account_id")?,
            posted: row.get("posted")?,
            amount: row.get("amount")?,
            description: row.get("description")?,
            pending: row.get::<_, i64>("pending")? != 0,
            category_id: row.get("category_id")?,
            reimburser_id: row.get("reimburser_id")?,
            reimbursed_at: row.get("reimbursed_at")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }

    pub fn amount_dollars(&self) -> f64 {
        self.amount as f64 / 100.0
    }

    /// Set the category for a transaction.
    pub fn set_category(conn: &Connection, tx_id: &str, category_id: i64) -> Result<()> {
        conn.execute(
            "UPDATE transactions SET category_id = ?1 WHERE id = ?2",
            rusqlite::params![category_id, tx_id],
        )?;
        Ok(())
    }

    /// Mark a transaction as reimbursable by the given reimburser. Resets reimbursed_at.
    pub fn set_reimburser(conn: &Connection, tx_id: &str, reimburser_id: i64) -> Result<bool> {
        let updated = conn.execute(
            "UPDATE transactions SET reimburser_id = ?1, reimbursed_at = NULL WHERE id = ?2",
            rusqlite::params![reimburser_id, tx_id],
        )?;
        Ok(updated > 0)
    }

    /// Mark a reimbursable transaction as paid back (sets reimbursed_at to the given timestamp).
    pub fn set_reimbursed_at(conn: &Connection, tx_id: &str, ts: Option<i64>) -> Result<bool> {
        let updated = conn.execute(
            "UPDATE transactions SET reimbursed_at = ?1 WHERE id = ?2",
            rusqlite::params![ts, tx_id],
        )?;
        Ok(updated > 0)
    }

    /// Clear all reimbursement state on a transaction.
    pub fn clear_reimburser(conn: &Connection, tx_id: &str) -> Result<bool> {
        let updated = conn.execute(
            "UPDATE transactions SET reimburser_id = NULL, reimbursed_at = NULL WHERE id = ?1",
            rusqlite::params![tx_id],
        )?;
        Ok(updated > 0)
    }
}

/// A transaction with display-ready fields for UI/listing.
#[derive(Debug, Clone)]
pub struct TransactionRow {
    pub id: String,
    pub date: String,
    pub amount: f64,
    pub description: String,
    pub category: Option<String>,
    pub account_name: String,
    pub pending: bool,
    pub reimburser: Option<String>,
    pub reimbursed_at: Option<i64>,
}

/// Filter for listing reimbursable transactions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReimbursableFilter {
    All,
    Pending,
    Paid,
}

impl TransactionRow {
    /// Find all transactions, optionally filtered by account.
    pub fn find_all(
        conn: &Connection,
        account_filter: Option<&str>,
        limit: usize,
    ) -> Result<Vec<TransactionRow>> {
        use chrono::{TimeZone, Utc};

        let (query, params): (String, Vec<String>) = if let Some(filter) = account_filter {
            (
                format!(
                    "SELECT t.id, t.posted, t.amount, t.description, t.pending, c.name as category,
                            COALESCE(a.nickname, a.name) as account_name,
                            r.name as reimburser, t.reimbursed_at
                     FROM transactions t
                     LEFT JOIN categories c ON t.category_id = c.id
                     LEFT JOIN reimbursers r ON t.reimburser_id = r.id
                     JOIN accounts a ON t.account_id = a.id
                     WHERE a.id LIKE '%' || ?1 || '%' OR a.nickname LIKE '%' || ?1 || '%' OR a.name LIKE '%' || ?1 || '%'
                     ORDER BY t.posted DESC
                     LIMIT {}", limit),
                vec![filter.to_string()]
            )
        } else {
            (
                format!(
                    "SELECT t.id, t.posted, t.amount, t.description, t.pending, c.name as category,
                            COALESCE(a.nickname, a.name) as account_name,
                            r.name as reimburser, t.reimbursed_at
                     FROM transactions t
                     LEFT JOIN categories c ON t.category_id = c.id
                     LEFT JOIN reimbursers r ON t.reimburser_id = r.id
                     JOIN accounts a ON t.account_id = a.id
                     ORDER BY t.posted DESC
                     LIMIT {}",
                    limit
                ),
                vec![],
            )
        };

        let mut stmt = conn.prepare(&query)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), |row| {
            let posted: i64 = row.get(1)?;
            let date = Utc
                .timestamp_opt(posted, 0)
                .single()
                .map(|dt| dt.format("%Y-%m-%d").to_string())
                .unwrap_or_default();

            Ok(TransactionRow {
                id: row.get(0)?,
                date,
                amount: row.get::<_, i64>(2)? as f64 / 100.0,
                description: row.get(3)?,
                pending: row.get::<_, i64>(4)? != 0,
                category: row.get(5)?,
                account_name: row.get(6)?,
                reimburser: row.get(7)?,
                reimbursed_at: row.get(8)?,
            })
        })?;

        let transactions = rows.collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(transactions)
    }

    /// Find transactions marked as reimbursable, with optional status and entity filters.
    pub fn find_reimbursable(
        conn: &Connection,
        filter: ReimbursableFilter,
        entity: Option<&str>,
    ) -> Result<Vec<TransactionRow>> {
        use chrono::{TimeZone, Utc};

        let mut where_clauses = vec!["t.reimburser_id IS NOT NULL".to_string()];
        let mut params: Vec<String> = Vec::new();

        match filter {
            ReimbursableFilter::Pending => {
                where_clauses.push("t.reimbursed_at IS NULL".to_string())
            }
            ReimbursableFilter::Paid => {
                where_clauses.push("t.reimbursed_at IS NOT NULL".to_string())
            }
            ReimbursableFilter::All => {}
        }

        if let Some(name) = entity {
            where_clauses.push(format!("LOWER(r.name) = LOWER(?{})", params.len() + 1));
            params.push(name.to_string());
        }

        let query = format!(
            "SELECT t.id, t.posted, t.amount, t.description, t.pending, c.name as category,
                    COALESCE(a.nickname, a.name) as account_name,
                    r.name as reimburser, t.reimbursed_at
             FROM transactions t
             LEFT JOIN categories c ON t.category_id = c.id
             JOIN reimbursers r ON t.reimburser_id = r.id
             JOIN accounts a ON t.account_id = a.id
             WHERE {}
             ORDER BY r.name, t.posted DESC",
            where_clauses.join(" AND ")
        );

        let mut stmt = conn.prepare(&query)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), |row| {
            let posted: i64 = row.get(1)?;
            let date = Utc
                .timestamp_opt(posted, 0)
                .single()
                .map(|dt| dt.format("%Y-%m-%d").to_string())
                .unwrap_or_default();

            Ok(TransactionRow {
                id: row.get(0)?,
                date,
                amount: row.get::<_, i64>(2)? as f64 / 100.0,
                description: row.get(3)?,
                pending: row.get::<_, i64>(4)? != 0,
                category: row.get(5)?,
                account_name: row.get(6)?,
                reimburser: row.get(7)?,
                reimbursed_at: row.get(8)?,
            })
        })?;

        let transactions = rows.collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(transactions)
    }
}

#[derive(Debug, Clone)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub created_at: i64,
}

impl Category {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            name: row.get("name")?,
            parent_id: row.get("parent_id")?,
            created_at: row.get("created_at")?,
        })
    }

    /// Find all categories, ordered by name.
    pub fn find_all(conn: &Connection) -> Result<Vec<Category>> {
        let mut stmt =
            conn.prepare("SELECT id, name, parent_id, created_at FROM categories ORDER BY name")?;
        let categories = stmt
            .query_map([], Category::from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(categories)
    }

    /// Find all categories as (id, name) tuples for selection dialogs.
    pub fn find_all_for_select(conn: &Connection) -> Result<Vec<(i64, String)>> {
        let mut stmt = conn.prepare("SELECT id, name FROM categories ORDER BY name")?;
        let items = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(items)
    }

    /// Find all categories as (id_string, name) tuples for selection dialogs.
    pub fn find_all_for_select_strings(conn: &Connection) -> Result<Vec<(String, String)>> {
        let mut stmt = conn.prepare("SELECT id, name FROM categories ORDER BY name")?;
        let items = stmt
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                let name: String = row.get(1)?;
                Ok((id.to_string(), name))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(items)
    }
}

#[derive(Debug, Clone)]
pub struct MerchantRule {
    pub pattern: String,
    pub category_id: i64,
    pub created_at: i64,
}

impl MerchantRule {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            pattern: row.get("pattern")?,
            category_id: row.get("category_id")?,
            created_at: row.get("created_at")?,
        })
    }

    /// Insert or update a merchant rule.
    pub fn upsert(
        conn: &Connection,
        pattern: &str,
        match_mode: &str,
        category_id: i64,
    ) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "INSERT OR REPLACE INTO merchant_rules (pattern, match_mode, category_id, created_at) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![pattern, match_mode, category_id, now],
        )?;
        Ok(())
    }

    /// Delete a merchant rule by pattern.
    pub fn delete_by_pattern(conn: &Connection, pattern: &str) -> Result<()> {
        conn.execute("DELETE FROM merchant_rules WHERE pattern = ?1", [pattern])?;
        Ok(())
    }

    /// Apply a rule to all transactions with matching descriptions.
    /// Returns the number of transactions updated.
    pub fn apply_to_transactions(
        conn: &Connection,
        pattern: &str,
        match_mode: &str,
        category_id: i64,
    ) -> Result<usize> {
        let pattern_lower = pattern.to_lowercase();

        // For substring matching, use SQL LIKE which is more reliable
        if match_mode != "token" {
            let like_pattern = format!("%{}%", pattern_lower);
            let count = conn.execute(
                "UPDATE transactions SET category_id = ?1 WHERE LOWER(description) LIKE ?2",
                rusqlite::params![category_id, like_pattern],
            )?;
            return Ok(count);
        }

        // For token matching, we need to do it in Rust since SQL can't easily do word-boundary matching
        let tx_ids: Vec<(String, String)> = {
            let mut stmt = conn.prepare("SELECT id, description FROM transactions")?;
            stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .filter_map(|r| r.ok())
                .collect()
        };

        let mut count = 0;
        for (tx_id, description) in tx_ids {
            let desc_lower = description.to_lowercase();
            // Token match: check if any word starts with the pattern
            if desc_lower
                .split_whitespace()
                .any(|word| word.starts_with(&pattern_lower))
            {
                conn.execute(
                    "UPDATE transactions SET category_id = ?1 WHERE id = ?2",
                    rusqlite::params![category_id, tx_id],
                )?;
                count += 1;
            }
        }

        Ok(count)
    }
}

/// A rule with its associated category name (for display purposes).
#[derive(Debug, Clone)]
pub struct RuleRow {
    pub pattern: String,
    pub match_mode: String,
    pub category: String,
}

impl RuleRow {
    /// Find all rules with their category names, ordered by pattern.
    pub fn find_all(conn: &Connection) -> Result<Vec<RuleRow>> {
        let mut stmt = conn.prepare(
            "SELECT mr.pattern, mr.match_mode, c.name
             FROM merchant_rules mr
             JOIN categories c ON mr.category_id = c.id
             ORDER BY mr.pattern",
        )?;
        let rules = stmt
            .query_map([], |row| {
                Ok(RuleRow {
                    pattern: row.get(0)?,
                    match_mode: row.get(1)?,
                    category: row.get(2)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rules)
    }

    /// Find a rule matching a transaction description.
    pub fn find_for_description(conn: &Connection, description: &str) -> Result<Option<RuleRow>> {
        let desc_lower = description.to_lowercase();

        let mut stmt = conn.prepare(
            "SELECT mr.pattern, mr.match_mode, c.name
             FROM merchant_rules mr
             JOIN categories c ON mr.category_id = c.id",
        )?;

        let rules: Vec<RuleRow> = stmt
            .query_map([], |row| {
                Ok(RuleRow {
                    pattern: row.get(0)?,
                    match_mode: row.get(1)?,
                    category: row.get(2)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        for rule in rules {
            let pattern_lower = rule.pattern.to_lowercase();
            let matches = if rule.match_mode == "token" {
                desc_lower
                    .split_whitespace()
                    .any(|word| word.starts_with(&pattern_lower))
            } else {
                desc_lower.contains(&pattern_lower)
            };
            if matches {
                return Ok(Some(rule));
            }
        }
        Ok(None)
    }
}

#[derive(Debug, Clone)]
pub struct Holding {
    pub id: String,
    pub account_id: String,
    pub symbol: Option<String>,
    pub description: Option<String>,
    pub shares: String,
    pub price: Option<i64>,
    pub cost_basis: Option<i64>,
    pub market_value: Option<i64>,
    pub currency: String,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Holding {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            account_id: row.get("account_id")?,
            symbol: row.get("symbol")?,
            description: row.get("description")?,
            shares: row.get("shares")?,
            price: row.get("price")?,
            cost_basis: row.get("cost_basis")?,
            market_value: row.get("market_value")?,
            currency: row.get("currency")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }

    pub fn price_dollars(&self) -> Option<f64> {
        self.price.map(|cents| cents as f64 / 100.0)
    }

    pub fn cost_basis_dollars(&self) -> Option<f64> {
        self.cost_basis.map(|cents| cents as f64 / 100.0)
    }

    pub fn market_value_dollars(&self) -> Option<f64> {
        self.market_value.map(|cents| cents as f64 / 100.0)
    }

    /// Find a holding by ID prefix or symbol (case-insensitive).
    pub fn find_by_query(conn: &Connection, query: &str) -> Result<Option<Holding>> {
        let query_upper = query.to_uppercase();
        let holding = conn
            .query_row(
                "SELECT id, account_id, symbol, description, shares, price, cost_basis, market_value, currency, created_at, updated_at
                 FROM holdings
                 WHERE id = ?1 OR id LIKE '%:' || ?1 OR id LIKE ?1 || '%'
                 LIMIT 1",
                [&query_upper],
                Holding::from_row,
            )
            .optional()?;
        Ok(holding)
    }

    /// Find all holdings, ordered by market value descending.
    pub fn find_all(conn: &Connection) -> Result<Vec<Holding>> {
        let mut stmt = conn.prepare(
            "SELECT id, account_id, symbol, description, shares, price, cost_basis, market_value, currency, created_at, updated_at
             FROM holdings
             ORDER BY market_value DESC NULLS LAST"
        )?;
        let holdings = stmt
            .query_map([], Holding::from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(holdings)
    }

    /// Find holdings filtered by account (partial match on id, nickname, or name).
    pub fn find_by_account_filter(conn: &Connection, account_filter: &str) -> Result<Vec<Holding>> {
        let mut stmt = conn.prepare(
            "SELECT h.id, h.account_id, h.symbol, h.description, h.shares, h.price, h.cost_basis, h.market_value, h.currency, h.created_at, h.updated_at
             FROM holdings h
             JOIN accounts a ON h.account_id = a.id
             WHERE a.id LIKE '%' || ?1 || '%' OR a.nickname LIKE '%' || ?1 || '%' OR a.name LIKE '%' || ?1 || '%'
             ORDER BY h.market_value DESC NULLS LAST"
        )?;
        let holdings = stmt
            .query_map([account_filter], Holding::from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(holdings)
    }
}

#[derive(Debug, Clone)]
pub struct Asset {
    pub id: i64,
    pub name: String,
    pub asset_type: String,
    pub description: Option<String>,
    pub value: Option<i64>,
    pub cost_basis: Option<i64>,
    pub currency: String,
    pub acquired_date: Option<i64>,
    pub metadata: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Asset {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            name: row.get("name")?,
            asset_type: row.get("asset_type")?,
            description: row.get("description")?,
            value: row.get("value")?,
            cost_basis: row.get("cost_basis")?,
            currency: row.get("currency")?,
            acquired_date: row.get("acquired_date")?,
            metadata: row.get("metadata")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }

    pub fn value_dollars(&self) -> Option<f64> {
        self.value.map(|cents| cents as f64 / 100.0)
    }

    pub fn cost_basis_dollars(&self) -> Option<f64> {
        self.cost_basis.map(|cents| cents as f64 / 100.0)
    }

    /// Find all assets, ordered by type then value descending.
    pub fn find_all(conn: &Connection) -> Result<Vec<Asset>> {
        let mut stmt = conn.prepare(
            "SELECT id, name, asset_type, description, value, cost_basis, currency, acquired_date, metadata, created_at, updated_at
             FROM assets
             ORDER BY asset_type, value DESC NULLS LAST"
        )?;
        let assets = stmt
            .query_map([], Asset::from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(assets)
    }
}

#[derive(Debug, Clone)]
pub struct RewardPoints {
    pub id: i64,
    pub program: String,
    pub points: i64,
    pub note: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl RewardPoints {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            program: row.get("program")?,
            points: row.get("points")?,
            note: row.get("note")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }

    /// Find all reward points programs, ordered by program name.
    pub fn find_all(conn: &Connection) -> Result<Vec<RewardPoints>> {
        let mut stmt = conn.prepare(
            "SELECT id, program, points, note, created_at, updated_at
             FROM rewards_points
             ORDER BY program",
        )?;
        let points = stmt
            .query_map([], RewardPoints::from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(points)
    }

    /// Find a reward points program by name (case-insensitive).
    pub fn find_by_program(conn: &Connection, program: &str) -> Result<Option<RewardPoints>> {
        let result = conn
            .query_row(
                "SELECT id, program, points, note, created_at, updated_at
                 FROM rewards_points
                 WHERE LOWER(program) = LOWER(?1)",
                [program],
                RewardPoints::from_row,
            )
            .optional()?;
        Ok(result)
    }
}

/// A detected recurring transaction pattern.
#[derive(Debug, Clone)]
pub struct RecurringPattern {
    pub merchant: String,
    pub category: Option<String>,
    pub frequency: String, // "monthly" or "bi-monthly"
    pub avg_amount: f64,   // average amount in dollars
    pub occurrences: usize,
    pub last_date: String,
}

impl RecurringPattern {
    /// Detect recurring transaction patterns from transaction history.
    /// Looks for transactions with similar descriptions occurring at regular intervals.
    pub fn detect(conn: &Connection) -> Result<Vec<RecurringPattern>> {
        use chrono::{TimeZone, Utc};
        use std::collections::HashMap;

        // Fetch all non-pending transactions with their details
        let mut stmt = conn.prepare(
            "SELECT t.description, t.posted, t.amount, c.name as category
             FROM transactions t
             LEFT JOIN categories c ON t.category_id = c.id
             WHERE t.pending = 0
             ORDER BY t.posted ASC",
        )?;

        // Group transactions by normalized merchant name
        let mut groups: HashMap<String, Vec<(i64, i64, Option<String>)>> = HashMap::new();

        let rows = stmt.query_map([], |row| {
            let description: String = row.get(0)?;
            let posted: i64 = row.get(1)?;
            let amount: i64 = row.get(2)?;
            let category: Option<String> = row.get(3)?;
            Ok((description, posted, amount, category))
        })?;

        for row in rows {
            let (description, posted, amount, category) = row?;
            // Normalize: lowercase, stop at special chars, take first 25 chars, trim
            let normalized = description
                .to_lowercase()
                .chars()
                .take_while(|c| c.is_alphanumeric() || c.is_whitespace())
                .take(25)
                .collect::<String>()
                .trim()
                .to_string();

            if !normalized.is_empty() {
                groups
                    .entry(normalized)
                    .or_default()
                    .push((posted, amount, category));
            }
        }

        let mut patterns = Vec::new();

        for (merchant, mut transactions) in groups {
            // Need at least 3 occurrences to detect a pattern
            if transactions.len() < 3 {
                continue;
            }

            // Sort by date (should already be, but ensure)
            transactions.sort_by_key(|(posted, _, _)| *posted);

            // Calculate intervals between consecutive transactions (in days)
            let intervals: Vec<i64> = transactions
                .windows(2)
                .map(|w| (w[1].0 - w[0].0) / 86400) // seconds to days
                .collect();

            // Check if intervals are consistent
            if let Some(frequency) = Self::detect_frequency(&intervals) {
                let amounts: Vec<f64> = transactions
                    .iter()
                    .map(|(_, amt, _)| *amt as f64 / 100.0)
                    .collect();
                let avg_amount = amounts.iter().sum::<f64>() / amounts.len() as f64;

                // Get category from most recent transaction
                let category = transactions.last().and_then(|(_, _, cat)| cat.clone());

                // Format last date
                let last_posted = transactions.last().map(|(p, _, _)| *p).unwrap_or(0);
                let last_date = Utc
                    .timestamp_opt(last_posted, 0)
                    .single()
                    .map(|dt| dt.format("%Y-%m-%d").to_string())
                    .unwrap_or_default();

                // Capitalize first letter of merchant for display
                let merchant_display = if let Some(first) = merchant.chars().next() {
                    first.to_uppercase().to_string() + &merchant[first.len_utf8()..]
                } else {
                    merchant
                };

                patterns.push(RecurringPattern {
                    merchant: merchant_display,
                    category,
                    frequency,
                    avg_amount,
                    occurrences: transactions.len(),
                    last_date,
                });
            }
        }

        // Exclude Income, Transfers, and Investments categories
        let excluded_categories = ["income", "transfers", "investments"];
        patterns.retain(|p| {
            p.category
                .as_ref()
                .map(|c| {
                    !excluded_categories
                        .iter()
                        .any(|exc| c.to_lowercase() == *exc)
                })
                .unwrap_or(true) // Keep uncategorized
        });

        // Sort by last date (most recent first)
        patterns.sort_by(|a, b| b.last_date.cmp(&a.last_date));

        Ok(patterns)
    }

    /// Detect if intervals match a known frequency pattern.
    /// Returns Some("monthly") or Some("bi-monthly") if pattern detected.
    fn detect_frequency(intervals: &[i64]) -> Option<String> {
        if intervals.is_empty() {
            return None;
        }

        let avg_interval = intervals.iter().sum::<i64>() as f64 / intervals.len() as f64;

        // Calculate standard deviation to check consistency
        let variance = intervals
            .iter()
            .map(|&i| {
                let diff = i as f64 - avg_interval;
                diff * diff
            })
            .sum::<f64>()
            / intervals.len() as f64;
        let std_dev = variance.sqrt();

        // Allow some tolerance in interval consistency (up to 7 days std dev)
        if std_dev > 7.0 {
            return None;
        }

        // Monthly: 25-35 days average
        if (25.0..=35.0).contains(&avg_interval) {
            return Some("monthly".to_string());
        }

        // Bi-monthly: 55-70 days average
        if (55.0..=70.0).contains(&avg_interval) {
            return Some("bi-monthly".to_string());
        }

        None
    }
}

#[derive(Debug, Clone)]
pub struct Reimburser {
    pub id: i64,
    pub name: String,
    pub created_at: i64,
}

impl Reimburser {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            name: row.get("name")?,
            created_at: row.get("created_at")?,
        })
    }

    /// Find all reimbursers, ordered by name.
    pub fn find_all(conn: &Connection) -> Result<Vec<Reimburser>> {
        let mut stmt =
            conn.prepare("SELECT id, name, created_at FROM reimbursers ORDER BY name")?;
        let items = stmt
            .query_map([], Reimburser::from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(items)
    }

    /// Find a reimburser by name (case-insensitive).
    pub fn find_by_name(conn: &Connection, name: &str) -> Result<Option<Reimburser>> {
        let result = conn
            .query_row(
                "SELECT id, name, created_at FROM reimbursers WHERE name = ?1 COLLATE NOCASE",
                [name],
                Reimburser::from_row,
            )
            .optional()?;
        Ok(result)
    }

    /// Find all reimbursers as (id, name) tuples for selection dialogs.
    pub fn find_all_for_select(conn: &Connection) -> Result<Vec<(i64, String)>> {
        let mut stmt = conn.prepare("SELECT id, name FROM reimbursers ORDER BY name")?;
        let items = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(items)
    }

    /// Insert a new reimburser. Errors if the name already exists.
    pub fn insert(conn: &Connection, name: &str) -> Result<i64> {
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "INSERT INTO reimbursers (name, created_at) VALUES (?1, ?2)",
            rusqlite::params![name, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Delete a reimburser by name. Errors if any transaction still references it.
    pub fn delete_by_name(conn: &Connection, name: &str) -> Result<()> {
        let reimburser = Self::find_by_name(conn, name)?
            .ok_or_else(|| anyhow::anyhow!("Reimburser '{}' not found", name))?;

        let referenced: i64 = conn.query_row(
            "SELECT COUNT(*) FROM transactions WHERE reimburser_id = ?1",
            [reimburser.id],
            |row| row.get(0),
        )?;
        if referenced > 0 {
            anyhow::bail!(
                "Cannot remove '{}': {} transaction(s) still reference it. Clear them first with `pint reimburse <tx-id> --clear`.",
                name,
                referenced
            );
        }

        conn.execute("DELETE FROM reimbursers WHERE id = ?1", [reimburser.id])?;
        Ok(())
    }
}
