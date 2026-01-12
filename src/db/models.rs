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
             ORDER BY manual DESC, account_type, COALESCE(nickname, name)"
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
}

impl TransactionRow {
    /// Find all transactions, optionally filtered by account.
    pub fn find_all(conn: &Connection, account_filter: Option<&str>, limit: usize) -> Result<Vec<TransactionRow>> {
        use chrono::{TimeZone, Utc};

        let (query, params): (String, Vec<String>) = if let Some(filter) = account_filter {
            (
                format!(
                    "SELECT t.id, t.posted, t.amount, t.description, t.pending, c.name as category,
                            COALESCE(a.nickname, a.name) as account_name
                     FROM transactions t
                     LEFT JOIN categories c ON t.category_id = c.id
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
                            COALESCE(a.nickname, a.name) as account_name
                     FROM transactions t
                     LEFT JOIN categories c ON t.category_id = c.id
                     JOIN accounts a ON t.account_id = a.id
                     ORDER BY t.posted DESC
                     LIMIT {}", limit),
                vec![]
            )
        };

        let mut stmt = conn.prepare(&query)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), |row| {
            let posted: i64 = row.get(1)?;
            let date = Utc.timestamp_opt(posted, 0)
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
        let mut stmt = conn.prepare(
            "SELECT id, name, parent_id, created_at FROM categories ORDER BY name"
        )?;
        let categories = stmt
            .query_map([], Category::from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(categories)
    }

    /// Find all categories as (id, name) tuples for selection dialogs.
    pub fn find_all_for_select(conn: &Connection) -> Result<Vec<(i64, String)>> {
        let mut stmt = conn.prepare(
            "SELECT id, name FROM categories ORDER BY name"
        )?;
        let items = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(items)
    }

    /// Find all categories as (id_string, name) tuples for selection dialogs.
    pub fn find_all_for_select_strings(conn: &Connection) -> Result<Vec<(String, String)>> {
        let mut stmt = conn.prepare(
            "SELECT id, name FROM categories ORDER BY name"
        )?;
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
    pub fn upsert(conn: &Connection, pattern: &str, match_mode: &str, category_id: i64) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "INSERT OR REPLACE INTO merchant_rules (pattern, match_mode, category_id, created_at) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![pattern, match_mode, category_id, now],
        )?;
        Ok(())
    }

    /// Delete a merchant rule by pattern.
    pub fn delete_by_pattern(conn: &Connection, pattern: &str) -> Result<()> {
        conn.execute(
            "DELETE FROM merchant_rules WHERE pattern = ?1",
            [pattern],
        )?;
        Ok(())
    }

    /// Apply a rule to all transactions with matching descriptions.
    /// Returns the number of transactions updated.
    pub fn apply_to_transactions(conn: &Connection, pattern: &str, match_mode: &str, category_id: i64) -> Result<usize> {
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
            let mut stmt = conn.prepare(
                "SELECT id, description FROM transactions"
            )?;
            stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .filter_map(|r| r.ok())
                .collect()
        };

        let mut count = 0;
        for (tx_id, description) in tx_ids {
            let desc_lower = description.to_lowercase();
            // Token match: check if any word starts with the pattern
            if desc_lower.split_whitespace().any(|word| word.starts_with(&pattern_lower)) {
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
             ORDER BY mr.pattern"
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
             JOIN categories c ON mr.category_id = c.id"
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
                desc_lower.split_whitespace().any(|word| word.starts_with(&pattern_lower))
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

    /// Find all assets, ordered by value descending.
    pub fn find_all(conn: &Connection) -> Result<Vec<Asset>> {
        let mut stmt = conn.prepare(
            "SELECT id, name, asset_type, description, value, cost_basis, currency, acquired_date, metadata, created_at, updated_at
             FROM assets
             ORDER BY value DESC NULLS LAST"
        )?;
        let assets = stmt
            .query_map([], Asset::from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(assets)
    }
}
