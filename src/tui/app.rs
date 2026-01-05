use anyhow::Result;
use ratatui::widgets::TableState;
use rusqlite::Connection;

use crate::db::models::{Account, Asset, Category, Holding};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum View {
    Accounts,
    Transactions,
    Holdings,
    Assets,
    Rules,
}

impl View {
    pub const ALL: [View; 5] = [
        View::Accounts,
        View::Transactions,
        View::Holdings,
        View::Assets,
        View::Rules,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            View::Accounts => "Accounts",
            View::Transactions => "Transactions",
            View::Holdings => "Holdings",
            View::Assets => "Assets",
            View::Rules => "Rules",
        }
    }
}

pub struct App {
    pub conn: Connection,
    pub current_view: View,
    pub nav_index: usize,
    pub nav_focused: bool,

    // View data
    pub accounts: Vec<Account>,
    pub transactions: Vec<TransactionRow>,
    pub holdings: Vec<Holding>,
    pub assets: Vec<Asset>,
    pub categories: Vec<Category>,
    pub rules: Vec<RuleRow>,

    // Table state for scrolling per view
    pub accounts_state: TableState,
    pub transactions_state: TableState,
    pub holdings_state: TableState,
    pub assets_state: TableState,
    pub rules_state: TableState,

    // Search/filter state
    pub search_mode: bool,
    pub search_query: String,
    pub filter_account: Option<String>,

    // Status message
    pub status: Option<String>,
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct TransactionRow {
    pub id: String,
    pub date: String,
    pub amount: f64,
    pub description: String,
    pub category: Option<String>,
    pub account_name: String,
    pub pending: bool,
}

#[derive(Clone)]
pub struct RuleRow {
    pub pattern: String,
    pub match_mode: String,
    pub category: String,
}

impl App {
    pub fn new(conn: Connection) -> Self {
        Self {
            conn,
            current_view: View::Accounts,
            nav_index: 0,
            nav_focused: false,

            accounts: Vec::new(),
            transactions: Vec::new(),
            holdings: Vec::new(),
            assets: Vec::new(),
            categories: Vec::new(),
            rules: Vec::new(),

            accounts_state: TableState::default().with_selected(0),
            transactions_state: TableState::default().with_selected(0),
            holdings_state: TableState::default().with_selected(0),
            assets_state: TableState::default().with_selected(0),
            rules_state: TableState::default().with_selected(0),

            search_mode: false,
            search_query: String::new(),
            filter_account: None,

            status: None,
        }
    }

    pub fn load_data(&mut self) -> Result<()> {
        match self.current_view {
            View::Accounts => self.load_accounts()?,
            View::Transactions => self.load_transactions()?,
            View::Holdings => self.load_holdings()?,
            View::Assets => self.load_assets()?,
            View::Rules => self.load_rules()?,
        }
        Ok(())
    }

    fn load_accounts(&mut self) -> Result<()> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, nickname, institution, account_type, balance, balance_date, currency, manual, created_at, updated_at
             FROM accounts
             ORDER BY manual DESC, account_type, COALESCE(nickname, name)"
        )?;

        self.accounts = stmt
            .query_map([], |row| Account::from_row(row))?
            .collect::<Result<Vec<_>, _>>()?;

        if !self.accounts.is_empty() {
            let selected = self.accounts_state.selected().unwrap_or(0);
            if selected >= self.accounts.len() {
                self.accounts_state.select(Some(self.accounts.len() - 1));
            }
        }
        Ok(())
    }

    fn load_transactions(&mut self) -> Result<()> {
        use chrono::{TimeZone, Utc};

        let (query, params): (String, Vec<String>) = if let Some(ref account_filter) = self.filter_account {
            (
                "SELECT t.id, t.posted, t.amount, t.description, t.pending, c.name as category,
                        COALESCE(a.nickname, a.name) as account_name
                 FROM transactions t
                 LEFT JOIN categories c ON t.category_id = c.id
                 JOIN accounts a ON t.account_id = a.id
                 WHERE a.id LIKE '%' || ?1 || '%' OR a.nickname LIKE '%' || ?1 || '%' OR a.name LIKE '%' || ?1 || '%'
                 ORDER BY t.posted DESC
                 LIMIT 2000".to_string(),
                vec![account_filter.clone()]
            )
        } else {
            (
                "SELECT t.id, t.posted, t.amount, t.description, t.pending, c.name as category,
                        COALESCE(a.nickname, a.name) as account_name
                 FROM transactions t
                 LEFT JOIN categories c ON t.category_id = c.id
                 JOIN accounts a ON t.account_id = a.id
                 ORDER BY t.posted DESC
                 LIMIT 2000".to_string(),
                vec![]
            )
        };

        let mut stmt = self.conn.prepare(&query)?;

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

        self.transactions = rows.collect::<Result<Vec<_>, _>>()?;

        // Apply search filter
        if !self.search_query.is_empty() {
            let query_lower = self.search_query.to_lowercase();
            self.transactions.retain(|t| {
                t.description.to_lowercase().contains(&query_lower)
                    || t.category.as_ref().map(|c| c.to_lowercase().contains(&query_lower)).unwrap_or(false)
            });
        }

        if !self.transactions.is_empty() {
            let selected = self.transactions_state.selected().unwrap_or(0);
            if selected >= self.transactions.len() {
                self.transactions_state.select(Some(self.transactions.len() - 1));
            }
        }
        Ok(())
    }

    fn load_holdings(&mut self) -> Result<()> {
        let (query, params): (String, Vec<String>) = if let Some(ref account_filter) = self.filter_account {
            (
                "SELECT h.id, h.account_id, h.symbol, h.description, h.shares, h.price, h.cost_basis, h.market_value, h.currency, h.created_at, h.updated_at
                 FROM holdings h
                 JOIN accounts a ON h.account_id = a.id
                 WHERE a.id LIKE '%' || ?1 || '%' OR a.nickname LIKE '%' || ?1 || '%' OR a.name LIKE '%' || ?1 || '%'
                 ORDER BY h.market_value DESC NULLS LAST".to_string(),
                vec![account_filter.clone()]
            )
        } else {
            (
                "SELECT id, account_id, symbol, description, shares, price, cost_basis, market_value, currency, created_at, updated_at
                 FROM holdings
                 ORDER BY market_value DESC NULLS LAST".to_string(),
                vec![]
            )
        };

        let mut stmt = self.conn.prepare(&query)?;
        self.holdings = stmt
            .query_map(rusqlite::params_from_iter(params.iter()), |row| Holding::from_row(row))?
            .collect::<Result<Vec<_>, _>>()?;

        if !self.holdings.is_empty() {
            let selected = self.holdings_state.selected().unwrap_or(0);
            if selected >= self.holdings.len() {
                self.holdings_state.select(Some(self.holdings.len() - 1));
            }
        }
        Ok(())
    }

    fn load_assets(&mut self) -> Result<()> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, asset_type, description, value, cost_basis, currency, acquired_date, metadata, created_at, updated_at
             FROM assets
             ORDER BY value DESC NULLS LAST"
        )?;

        self.assets = stmt
            .query_map([], |row| Asset::from_row(row))?
            .collect::<Result<Vec<_>, _>>()?;

        if !self.assets.is_empty() {
            let selected = self.assets_state.selected().unwrap_or(0);
            if selected >= self.assets.len() {
                self.assets_state.select(Some(self.assets.len() - 1));
            }
        }
        Ok(())
    }

    fn load_rules(&mut self) -> Result<()> {
        // Load categories
        let mut cat_stmt = self.conn.prepare(
            "SELECT id, name, parent_id, created_at FROM categories ORDER BY name"
        )?;
        self.categories = cat_stmt
            .query_map([], |row| Category::from_row(row))?
            .collect::<Result<Vec<_>, _>>()?;

        // Load rules
        let mut rule_stmt = self.conn.prepare(
            "SELECT mr.pattern, mr.match_mode, c.name
             FROM merchant_rules mr
             JOIN categories c ON mr.category_id = c.id
             ORDER BY mr.pattern"
        )?;
        self.rules = rule_stmt
            .query_map([], |row| {
                Ok(RuleRow {
                    pattern: row.get(0)?,
                    match_mode: row.get(1)?,
                    category: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !self.rules.is_empty() {
            let selected = self.rules_state.selected().unwrap_or(0);
            if selected >= self.rules.len() {
                self.rules_state.select(Some(self.rules.len() - 1));
            }
        }
        Ok(())
    }

    pub fn toggle_focus(&mut self) {
        self.nav_focused = !self.nav_focused;
    }

    pub fn nav_up(&mut self) {
        if self.nav_index > 0 {
            self.nav_index -= 1;
        }
    }

    pub fn nav_down(&mut self) {
        if self.nav_index < View::ALL.len() - 1 {
            self.nav_index += 1;
        }
    }

    pub fn select_nav(&mut self) {
        self.current_view = View::ALL[self.nav_index];
        self.nav_focused = false;
        self.search_query.clear();
        self.filter_account = None;
    }

    pub fn list_up(&mut self) {
        let state = match self.current_view {
            View::Accounts => &mut self.accounts_state,
            View::Transactions => &mut self.transactions_state,
            View::Holdings => &mut self.holdings_state,
            View::Assets => &mut self.assets_state,
            View::Rules => &mut self.rules_state,
        };
        let selected = state.selected().unwrap_or(0);
        if selected > 0 {
            state.select(Some(selected - 1));
        }
    }

    pub fn list_down(&mut self) {
        let (state, len) = match self.current_view {
            View::Accounts => (&mut self.accounts_state, self.accounts.len()),
            View::Transactions => (&mut self.transactions_state, self.transactions.len()),
            View::Holdings => (&mut self.holdings_state, self.holdings.len()),
            View::Assets => (&mut self.assets_state, self.assets.len()),
            View::Rules => (&mut self.rules_state, self.rules.len()),
        };
        let selected = state.selected().unwrap_or(0);
        if len > 0 && selected < len - 1 {
            state.select(Some(selected + 1));
        }
    }

    pub fn list_page_up(&mut self) {
        const PAGE_SIZE: usize = 10;
        let state = match self.current_view {
            View::Accounts => &mut self.accounts_state,
            View::Transactions => &mut self.transactions_state,
            View::Holdings => &mut self.holdings_state,
            View::Assets => &mut self.assets_state,
            View::Rules => &mut self.rules_state,
        };
        let selected = state.selected().unwrap_or(0);
        state.select(Some(selected.saturating_sub(PAGE_SIZE)));
    }

    pub fn list_page_down(&mut self) {
        const PAGE_SIZE: usize = 10;
        let (state, len) = match self.current_view {
            View::Accounts => (&mut self.accounts_state, self.accounts.len()),
            View::Transactions => (&mut self.transactions_state, self.transactions.len()),
            View::Holdings => (&mut self.holdings_state, self.holdings.len()),
            View::Assets => (&mut self.assets_state, self.assets.len()),
            View::Rules => (&mut self.rules_state, self.rules.len()),
        };
        let selected = state.selected().unwrap_or(0);
        if len > 0 {
            state.select(Some((selected + PAGE_SIZE).min(len - 1)));
        }
    }

    pub fn select_item(&mut self) -> Result<()> {
        match self.current_view {
            View::Accounts => {
                let selected = self.accounts_state.selected().unwrap_or(0);
                if let Some(account) = self.accounts.get(selected) {
                    self.filter_account = Some(account.id.clone());

                    // Brokerage/retirement accounts show holdings, others show transactions
                    if account.account_type == "brokerage" || account.account_type == "retirement" {
                        self.current_view = View::Holdings;
                        self.nav_index = 2; // Holdings
                        self.holdings_state.select(Some(0));
                        self.load_holdings()?;
                    } else {
                        self.current_view = View::Transactions;
                        self.nav_index = 1; // Transactions
                        self.transactions_state.select(Some(0));
                        self.load_transactions()?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub fn is_searching(&self) -> bool {
        self.search_mode
    }

    pub fn start_search(&mut self) {
        self.search_mode = true;
        self.search_query.clear();
    }

    pub fn cancel_search(&mut self) {
        self.search_mode = false;
        self.search_query.clear();
        let _ = self.load_data();
    }

    pub fn search_input(&mut self, c: char) {
        self.search_query.push(c);
        let _ = self.load_data();
    }

    pub fn search_backspace(&mut self) {
        self.search_query.pop();
        let _ = self.load_data();
    }

    pub fn sync(&mut self) -> Result<()> {
        self.status = Some("Syncing...".to_string());
        // Note: sync requires reopening connection or running in separate process
        // For now, just reload data
        self.status = Some("Sync not available in TUI yet. Use 'pint sync' from command line.".to_string());
        Ok(())
    }

    /// Calculate total balance of all accounts in cents
    pub fn accounts_total(&self) -> i64 {
        self.accounts.iter().filter_map(|a| a.balance).sum()
    }

    /// Calculate total value of all assets in cents
    pub fn assets_total(&self) -> i64 {
        self.assets.iter().filter_map(|a| a.value).sum()
    }

    /// Calculate total market value of displayed holdings in cents
    pub fn holdings_total(&self) -> i64 {
        self.holdings.iter().filter_map(|h| h.market_value).sum()
    }
}

/// Format an amount in cents with thousand separators based on currency
pub fn format_amount(cents: i64, currency: &str) -> String {
    let dollars = cents as f64 / 100.0;
    let formatted = format!("{:.2}", dollars.abs());

    // Split into integer and decimal parts
    let parts: Vec<&str> = formatted.split('.').collect();
    let int_part = parts[0];
    let dec_part = parts.get(1).unwrap_or(&"00");

    // Add thousand separators
    let separator = if currency == "EUR" { ' ' } else { ',' };
    let with_separators: String = int_part
        .chars()
        .rev()
        .enumerate()
        .flat_map(|(i, c)| {
            if i > 0 && i % 3 == 0 {
                vec![separator, c]
            } else {
                vec![c]
            }
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let sign = if cents < 0 { "-" } else { "" };
    format!("{}{}.{}", sign, with_separators, dec_part)
}
