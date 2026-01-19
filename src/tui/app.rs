use anyhow::Result;
use ratatui::widgets::TableState;
use rusqlite::Connection;

use crate::db::models::{Account, Asset, Category, Holding, MerchantRule, RecurringPattern, RuleRow, Transaction, TransactionRow};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum View {
    Summary,
    Accounts,
    Transactions,
    Holdings,
    Assets,
    Recurring,
    Rules,
}

impl View {
    pub const ALL: [View; 7] = [
        View::Summary,
        View::Accounts,
        View::Transactions,
        View::Holdings,
        View::Assets,
        View::Recurring,
        View::Rules,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            View::Summary => "Summary",
            View::Accounts => "Accounts",
            View::Transactions => "Transactions",
            View::Holdings => "Holdings",
            View::Assets => "Assets",
            View::Recurring => "Recurring",
            View::Rules => "Rules",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            View::Summary => "📊",
            View::Accounts => "🏦",
            View::Transactions => "💵",
            View::Holdings => "📈",
            View::Assets => "🏡",
            View::Recurring => "🔄",
            View::Rules => "📜",
        }
    }

    pub fn label(&self) -> String {
        format!("{} {}", self.icon(), self.name())
    }
}

/// Type of dialog currently active
#[derive(Clone, PartialEq, Eq)]
pub enum DialogType {
    /// Non-interactive info/progress message
    Info {
        title: String,
        message: String,
    },
    /// Confirmation dialog (e.g., delete confirmation)
    Confirm {
        title: String,
        message: String,
    },
    /// Single text input
    Input {
        title: String,
        prompt: String,
    },
    /// Two text inputs (e.g., add account: name + type)
    TwoInputs {
        title: String,
        prompt1: String,
        prompt2: String,
        /// Which input is focused (0 or 1)
        focused: usize,
    },
    /// List selection dialog
    Select {
        title: String,
        items: Vec<(String, String)>, // (id, display_name)
        selected: usize,
    },
    /// Rule editor dialog (pattern + match mode + category)
    RuleEditor {
        title: String,
        /// 0 = pattern, 1 = match mode, 2 = category
        focused_field: usize,
        /// Match mode: 0 = substring, 1 = token
        match_mode: usize,
        /// Available categories (id, name)
        categories: Vec<(i64, String)>,
        /// Selected category index
        selected_category: usize,
    },
}

/// Dialog state
pub struct Dialog {
    pub dialog_type: DialogType,
    pub input1: String,
    pub input2: String,
    pub action: DialogAction,
    /// Cursor position within input1
    pub cursor1: usize,
    /// Cursor position within input2
    pub cursor2: usize,
}

/// What action to perform when dialog is confirmed
#[derive(Clone, PartialEq, Eq)]
pub enum DialogAction {
    AddAccount,
    RemoveAccount { account_id: String },
    RenameAccount { account_id: String },
    SetAccountType { account_id: String },
    FilterByAccount,
    CreateRule { tx_id: String },
    EditRule { tx_id: String, rule_pattern: String },
    CategorizeTransaction { tx_id: String },
}

/// Summary data aggregated by account type
#[derive(Default)]
pub struct SummaryData {
    pub cash: i64,        // checking + savings
    pub brokerage: i64,   // brokerage accounts
    pub retirement: i64,  // retirement accounts
    pub assets: i64,      // manual assets
    pub credit: i64,      // credit cards (negative)
    pub net_worth: i64,   // total
}

pub struct App {
    pub conn: Connection,
    pub current_view: View,
    pub nav_index: usize,
    pub nav_focused: bool,

    // View data
    pub summary: SummaryData,
    pub accounts: Vec<Account>,
    pub transactions: Vec<TransactionRow>,
    pub holdings: Vec<Holding>,
    pub assets: Vec<Asset>,
    pub recurring: Vec<RecurringPattern>,
    pub categories: Vec<Category>,
    pub rules: Vec<RuleRow>,

    // Table state for scrolling per view
    pub accounts_state: TableState,
    pub transactions_state: TableState,
    pub holdings_state: TableState,
    pub assets_state: TableState,
    pub recurring_state: TableState,
    pub rules_state: TableState,

    // Search/filter state
    pub search_mode: bool,
    pub search_query: String,
    pub filter_account: Option<String>,

    // Status message
    pub status: Option<String>,

    // Dialog state
    pub dialog: Option<Dialog>,

    // Track if user has interacted (to show context-specific help)
    pub has_interacted: bool,

    // Pending sync operation (triggered after next render)
    pub pending_sync: bool,
}


impl App {
    pub fn new(conn: Connection) -> Self {
        Self {
            conn,
            current_view: View::Summary,
            nav_index: 0,
            nav_focused: false,

            summary: SummaryData::default(),
            accounts: Vec::new(),
            transactions: Vec::new(),
            holdings: Vec::new(),
            assets: Vec::new(),
            recurring: Vec::new(),
            categories: Vec::new(),
            rules: Vec::new(),

            accounts_state: TableState::default().with_selected(0),
            transactions_state: TableState::default().with_selected(0),
            holdings_state: TableState::default().with_selected(0),
            assets_state: TableState::default().with_selected(0),
            recurring_state: TableState::default().with_selected(0),
            rules_state: TableState::default().with_selected(0),

            search_mode: false,
            search_query: String::new(),
            filter_account: None,

            status: None,

            dialog: None,
            has_interacted: false,
            pending_sync: false,
        }
    }

    pub fn load_data(&mut self) -> Result<()> {
        match self.current_view {
            View::Summary => self.load_summary()?,
            View::Accounts => self.load_accounts()?,
            View::Transactions => self.load_transactions()?,
            View::Holdings => self.load_holdings()?,
            View::Assets => self.load_assets()?,
            View::Recurring => self.load_recurring()?,
            View::Rules => self.load_rules()?,
        }
        Ok(())
    }

    fn load_summary(&mut self) -> Result<()> {
        // Load accounts to aggregate by type
        let accounts = Account::find_all(&self.conn)?;
        let assets = Asset::find_all(&self.conn)?;

        let mut cash: i64 = 0;
        let mut brokerage: i64 = 0;
        let mut retirement: i64 = 0;
        let mut credit: i64 = 0;

        for account in &accounts {
            let balance = account.balance.unwrap_or(0);
            match account.account_type.as_str() {
                "checking" | "savings" => cash += balance,
                "brokerage" => brokerage += balance,
                "retirement" => retirement += balance,
                "credit" => credit += balance, // already negative
                _ => {} // loan, unknown, etc. - skip for now
            }
        }

        let assets_total: i64 = assets.iter().filter_map(|a| a.value).sum();

        let net_worth = cash + brokerage + retirement + assets_total + credit;

        self.summary = SummaryData {
            cash,
            brokerage,
            retirement,
            assets: assets_total,
            credit,
            net_worth,
        };

        // Also load recurring for display in summary
        self.recurring = RecurringPattern::detect(&self.conn)?;

        Ok(())
    }

    fn load_accounts(&mut self) -> Result<()> {
        self.accounts = Account::find_all(&self.conn)?;

        if !self.accounts.is_empty() {
            let selected = self.accounts_state.selected().unwrap_or(0);
            if selected >= self.accounts.len() {
                self.accounts_state.select(Some(self.accounts.len() - 1));
            }
        }
        Ok(())
    }

    fn load_transactions(&mut self) -> Result<()> {
        self.transactions = TransactionRow::find_all(&self.conn, self.filter_account.as_deref(), 2000)?;

        // Apply search filter (UI-specific, in-memory)
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
        self.holdings = match &self.filter_account {
            Some(filter) => Holding::find_by_account_filter(&self.conn, filter)?,
            None => Holding::find_all(&self.conn)?,
        };

        if !self.holdings.is_empty() {
            let selected = self.holdings_state.selected().unwrap_or(0);
            if selected >= self.holdings.len() {
                self.holdings_state.select(Some(self.holdings.len() - 1));
            }
        }
        Ok(())
    }

    fn load_assets(&mut self) -> Result<()> {
        self.assets = Asset::find_all(&self.conn)?;

        if !self.assets.is_empty() {
            let selected = self.assets_state.selected().unwrap_or(0);
            if selected >= self.assets.len() {
                self.assets_state.select(Some(self.assets.len() - 1));
            }
        }
        Ok(())
    }

    fn load_recurring(&mut self) -> Result<()> {
        self.recurring = RecurringPattern::detect(&self.conn)?;

        if !self.recurring.is_empty() {
            let selected = self.recurring_state.selected().unwrap_or(0);
            if selected >= self.recurring.len() {
                self.recurring_state.select(Some(self.recurring.len() - 1));
            }
        }
        Ok(())
    }

    fn load_rules(&mut self) -> Result<()> {
        self.categories = Category::find_all(&self.conn)?;
        self.rules = RuleRow::find_all(&self.conn)?;

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
            View::Summary => return, // No list navigation in summary
            View::Accounts => &mut self.accounts_state,
            View::Transactions => &mut self.transactions_state,
            View::Holdings => &mut self.holdings_state,
            View::Assets => &mut self.assets_state,
            View::Recurring => &mut self.recurring_state,
            View::Rules => &mut self.rules_state,
        };
        let selected = state.selected().unwrap_or(0);
        if selected > 0 {
            state.select(Some(selected - 1));
        }
    }

    pub fn list_down(&mut self) {
        let (state, len) = match self.current_view {
            View::Summary => return, // No list navigation in summary
            View::Accounts => (&mut self.accounts_state, self.accounts.len()),
            View::Transactions => (&mut self.transactions_state, self.transactions.len()),
            View::Holdings => (&mut self.holdings_state, self.holdings.len()),
            View::Assets => (&mut self.assets_state, self.assets.len()),
            View::Recurring => (&mut self.recurring_state, self.recurring.len()),
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
            View::Summary => return, // No list navigation in summary
            View::Accounts => &mut self.accounts_state,
            View::Transactions => &mut self.transactions_state,
            View::Holdings => &mut self.holdings_state,
            View::Assets => &mut self.assets_state,
            View::Recurring => &mut self.recurring_state,
            View::Rules => &mut self.rules_state,
        };
        let selected = state.selected().unwrap_or(0);
        state.select(Some(selected.saturating_sub(PAGE_SIZE)));
    }

    pub fn list_page_down(&mut self) {
        const PAGE_SIZE: usize = 10;
        let (state, len) = match self.current_view {
            View::Summary => return, // No list navigation in summary
            View::Accounts => (&mut self.accounts_state, self.accounts.len()),
            View::Transactions => (&mut self.transactions_state, self.transactions.len()),
            View::Holdings => (&mut self.holdings_state, self.holdings.len()),
            View::Assets => (&mut self.assets_state, self.assets.len()),
            View::Recurring => (&mut self.recurring_state, self.recurring.len()),
            View::Rules => (&mut self.rules_state, self.rules.len()),
        };
        let selected = state.selected().unwrap_or(0);
        if len > 0 {
            state.select(Some((selected + PAGE_SIZE).min(len - 1)));
        }
    }

    pub fn select_item(&mut self) -> Result<()> {
        if self.current_view == View::Accounts {
            let selected = self.accounts_state.selected().unwrap_or(0);
            if let Some(account) = self.accounts.get(selected) {
                self.filter_account = Some(account.id.clone());

                // Brokerage/retirement accounts show holdings, others show transactions
                if account.account_type == "brokerage" || account.account_type == "retirement" {
                    self.current_view = View::Holdings;
                    self.nav_index = 3; // Holdings (Summary=0, Accounts=1, Transactions=2, Holdings=3)
                    self.holdings_state.select(Some(0));
                    self.load_holdings()?;
                } else {
                    self.current_view = View::Transactions;
                    self.nav_index = 2; // Transactions
                    self.transactions_state.select(Some(0));
                    self.load_transactions()?;
                }
            }
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

    // Dialog methods

    pub fn has_dialog(&self) -> bool {
        self.dialog.is_some()
    }

    pub fn show_dialog(&mut self, dialog_type: DialogType, action: DialogAction) {
        self.show_dialog_with_value(dialog_type, action, String::new());
    }

    pub fn show_dialog_with_value(&mut self, dialog_type: DialogType, action: DialogAction, prefill: String) {
        let cursor1 = prefill.len();
        self.dialog = Some(Dialog {
            dialog_type,
            input1: prefill,
            input2: String::new(),
            action,
            cursor1,
            cursor2: 0,
        });
    }

    pub fn close_dialog(&mut self) {
        self.dialog = None;
    }

    pub fn dialog_input(&mut self, c: char) {
        if let Some(ref mut dialog) = self.dialog {
            match &mut dialog.dialog_type {
                DialogType::Info { .. } | DialogType::Confirm { .. } => {}
                DialogType::Select { selected, .. } => {
                    // Input goes to search filter, reset selection
                    dialog.input1.insert(dialog.cursor1, c);
                    dialog.cursor1 += 1;
                    *selected = 0;
                }
                DialogType::Input { .. } => {
                    dialog.input1.insert(dialog.cursor1, c);
                    dialog.cursor1 += 1;
                }
                DialogType::TwoInputs { focused, .. } => {
                    if *focused == 0 {
                        dialog.input1.insert(dialog.cursor1, c);
                        dialog.cursor1 += 1;
                    } else {
                        dialog.input2.insert(dialog.cursor2, c);
                        dialog.cursor2 += 1;
                    }
                }
                DialogType::RuleEditor { focused_field, .. } => {
                    // Only accept input when pattern field is focused
                    if *focused_field == 0 {
                        dialog.input1.insert(dialog.cursor1, c);
                        dialog.cursor1 += 1;
                    }
                }
            }
        }
    }

    pub fn dialog_backspace(&mut self) {
        if let Some(ref mut dialog) = self.dialog {
            match &mut dialog.dialog_type {
                DialogType::Info { .. } | DialogType::Confirm { .. } => {}
                DialogType::Select { selected, .. } => {
                    // Backspace in search filter, reset selection
                    if dialog.cursor1 > 0 {
                        dialog.cursor1 -= 1;
                        dialog.input1.remove(dialog.cursor1);
                        *selected = 0;
                    }
                }
                DialogType::Input { .. } => {
                    if dialog.cursor1 > 0 {
                        dialog.cursor1 -= 1;
                        dialog.input1.remove(dialog.cursor1);
                    }
                }
                DialogType::TwoInputs { focused, .. } => {
                    if *focused == 0 && dialog.cursor1 > 0 {
                        dialog.cursor1 -= 1;
                        dialog.input1.remove(dialog.cursor1);
                    } else if *focused == 1 && dialog.cursor2 > 0 {
                        dialog.cursor2 -= 1;
                        dialog.input2.remove(dialog.cursor2);
                    }
                }
                DialogType::RuleEditor { focused_field, .. } => {
                    if *focused_field == 0 && dialog.cursor1 > 0 {
                        dialog.cursor1 -= 1;
                        dialog.input1.remove(dialog.cursor1);
                    }
                }
            }
        }
    }

    pub fn dialog_cursor_left(&mut self) {
        if let Some(ref mut dialog) = self.dialog {
            match &dialog.dialog_type {
                DialogType::Info { .. } | DialogType::Confirm { .. } => {}
                DialogType::Select { .. } | DialogType::Input { .. } => {
                    if dialog.cursor1 > 0 {
                        dialog.cursor1 -= 1;
                    }
                }
                DialogType::TwoInputs { focused, .. } => {
                    if *focused == 0 && dialog.cursor1 > 0 {
                        dialog.cursor1 -= 1;
                    } else if *focused == 1 && dialog.cursor2 > 0 {
                        dialog.cursor2 -= 1;
                    }
                }
                DialogType::RuleEditor { focused_field, .. } => {
                    if *focused_field == 0 && dialog.cursor1 > 0 {
                        dialog.cursor1 -= 1;
                    }
                }
            }
        }
    }

    pub fn dialog_cursor_right(&mut self) {
        if let Some(ref mut dialog) = self.dialog {
            match &dialog.dialog_type {
                DialogType::Info { .. } | DialogType::Confirm { .. } => {}
                DialogType::Select { .. } | DialogType::Input { .. } => {
                    if dialog.cursor1 < dialog.input1.len() {
                        dialog.cursor1 += 1;
                    }
                }
                DialogType::TwoInputs { focused, .. } => {
                    if *focused == 0 && dialog.cursor1 < dialog.input1.len() {
                        dialog.cursor1 += 1;
                    } else if *focused == 1 && dialog.cursor2 < dialog.input2.len() {
                        dialog.cursor2 += 1;
                    }
                }
                DialogType::RuleEditor { focused_field, .. } => {
                    if *focused_field == 0 && dialog.cursor1 < dialog.input1.len() {
                        dialog.cursor1 += 1;
                    }
                }
            }
        }
    }

    pub fn dialog_select_up(&mut self) {
        if let Some(ref mut dialog) = self.dialog {
            match &mut dialog.dialog_type {
                DialogType::Select { selected, .. } => {
                    if *selected > 0 {
                        *selected -= 1;
                    }
                }
                DialogType::RuleEditor { focused_field, match_mode, selected_category, .. } => {
                    match *focused_field {
                        1 => {
                            // Match mode: toggle between 0 and 1
                            if *match_mode > 0 {
                                *match_mode = 0;
                            }
                        }
                        2 => {
                            // Category selection
                            if *selected_category > 0 {
                                *selected_category -= 1;
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    pub fn dialog_select_down(&mut self) {
        if let Some(ref mut dialog) = self.dialog {
            let filter = dialog.input1.to_lowercase();
            match &mut dialog.dialog_type {
                DialogType::Select { selected, items, .. } => {
                    // Count filtered items
                    let filtered_count = if filter.is_empty() {
                        items.len()
                    } else {
                        items.iter().filter(|(_, name)| name.to_lowercase().contains(&filter)).count()
                    };
                    if *selected < filtered_count.saturating_sub(1) {
                        *selected += 1;
                    }
                }
                DialogType::RuleEditor { focused_field, match_mode, selected_category, categories, .. } => {
                    match *focused_field {
                        1 => {
                            // Match mode: toggle between 0 and 1
                            if *match_mode < 1 {
                                *match_mode = 1;
                            }
                        }
                        2 => {
                            // Category selection
                            if *selected_category < categories.len().saturating_sub(1) {
                                *selected_category += 1;
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    pub fn dialog_next_field(&mut self) {
        if let Some(ref mut dialog) = self.dialog {
            match &mut dialog.dialog_type {
                DialogType::TwoInputs { focused, .. } => {
                    *focused = (*focused + 1) % 2;
                }
                DialogType::RuleEditor { focused_field, .. } => {
                    *focused_field = (*focused_field + 1) % 3;
                }
                _ => {}
            }
        }
    }

    pub fn dialog_confirm(&mut self) -> Result<()> {
        if let Some(dialog) = self.dialog.take() {
            match dialog.action {
                DialogAction::AddAccount => {
                    if !dialog.input1.is_empty() && !dialog.input2.is_empty() {
                        self.execute_add_account(&dialog.input1, &dialog.input2)?;
                    }
                }
                DialogAction::RemoveAccount { account_id } => {
                    self.execute_remove_account(&account_id)?;
                }
                DialogAction::RenameAccount { account_id } => {
                    let nickname = if dialog.input1.is_empty() { None } else { Some(dialog.input1.as_str()) };
                    self.execute_rename_account(&account_id, nickname)?;
                }
                DialogAction::SetAccountType { account_id } => {
                    if !dialog.input1.is_empty() {
                        self.execute_set_account_type(&account_id, &dialog.input1)?;
                    }
                }
                DialogAction::FilterByAccount => {
                    // For Select dialog, get the selected account ID from filtered list
                    if let DialogType::Select { items, selected, .. } = &dialog.dialog_type {
                        let filter = dialog.input1.to_lowercase();
                        let filtered_items: Vec<&(String, String)> = if filter.is_empty() {
                            items.iter().collect()
                        } else {
                            items.iter().filter(|(_, name)| name.to_lowercase().contains(&filter)).collect()
                        };
                        if let Some((account_id, _)) = filtered_items.get(*selected) {
                            self.filter_account = Some(account_id.to_string());
                            self.load_data()?;
                        }
                    }
                }
                DialogAction::CreateRule { tx_id } => {
                    if let DialogType::RuleEditor { match_mode, categories, selected_category, .. } = &dialog.dialog_type
                        && !dialog.input1.is_empty()
                        && let Some((category_id, category_name)) = categories.get(*selected_category)
                    {
                        let match_str = if *match_mode == 0 { "substring" } else { "token" };
                        self.execute_create_rule(&tx_id, &dialog.input1, match_str, *category_id, category_name)?;
                    }
                }
                DialogAction::EditRule { tx_id, rule_pattern } => {
                    if let DialogType::RuleEditor { match_mode, categories, selected_category, .. } = &dialog.dialog_type
                        && !dialog.input1.is_empty()
                        && let Some((category_id, category_name)) = categories.get(*selected_category)
                    {
                        let match_str = if *match_mode == 0 { "substring" } else { "token" };
                        self.execute_edit_rule(&tx_id, &rule_pattern, &dialog.input1, match_str, *category_id, category_name)?;
                    }
                }
                DialogAction::CategorizeTransaction { tx_id } => {
                    if let DialogType::Select { items, selected, .. } = &dialog.dialog_type {
                        // Filter items the same way as rendering to get the correct selection
                        let filter = dialog.input1.to_lowercase();
                        let filtered_items: Vec<&(String, String)> = if filter.is_empty() {
                            items.iter().collect()
                        } else {
                            items.iter().filter(|(_, name)| name.to_lowercase().contains(&filter)).collect()
                        };
                        if let Some((category_id_str, category_name)) = filtered_items.get(*selected)
                            && let Ok(category_id) = category_id_str.parse::<i64>()
                        {
                            self.execute_categorize_transaction(&tx_id, category_id, category_name)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    // Account actions

    pub fn show_add_account_dialog(&mut self) {
        self.show_dialog(
            DialogType::TwoInputs {
                title: "Add Manual Account".to_string(),
                prompt1: "Account name".to_string(),
                prompt2: "Type (checking/savings/credit/brokerage/retirement/loan)".to_string(),
                focused: 0,
            },
            DialogAction::AddAccount,
        );
    }

    pub fn show_remove_account_dialog(&mut self) {
        let selected = self.accounts_state.selected().unwrap_or(0);
        if let Some(account) = self.accounts.get(selected) {
            let account_id = account.id.clone();
            let name = account.display_name().to_string();
            self.show_dialog(
                DialogType::Confirm {
                    title: "Remove Account".to_string(),
                    message: format!("Remove '{}' and all its transactions?", name),
                },
                DialogAction::RemoveAccount { account_id },
            );
        }
    }

    pub fn show_rename_account_dialog(&mut self) {
        let selected = self.accounts_state.selected().unwrap_or(0);
        if let Some(account) = self.accounts.get(selected) {
            let account_id = account.id.clone();
            // Prefill with current nickname, or name if no nickname
            let current = account.nickname.clone().unwrap_or_else(|| account.name.clone());
            self.show_dialog_with_value(
                DialogType::Input {
                    title: "Rename Account".to_string(),
                    prompt: "New nickname (empty to clear)".to_string(),
                },
                DialogAction::RenameAccount { account_id },
                current,
            );
        }
    }

    pub fn show_set_type_dialog(&mut self) {
        let selected = self.accounts_state.selected().unwrap_or(0);
        if let Some(account) = self.accounts.get(selected) {
            let account_id = account.id.clone();
            self.show_dialog(
                DialogType::Input {
                    title: "Set Account Type".to_string(),
                    prompt: "Type (checking/savings/credit/brokerage/retirement/loan)".to_string(),
                },
                DialogAction::SetAccountType { account_id },
            );
        }
    }

    fn execute_add_account(&mut self, name: &str, account_type: &str) -> Result<()> {
        crate::commands::accounts::add_quiet(name, account_type, true)?;
        self.load_accounts()?;
        self.status = Some(format!("Added account '{}'", name));
        Ok(())
    }

    fn execute_remove_account(&mut self, account_id: &str) -> Result<()> {
        crate::commands::accounts::remove_quiet(account_id, true)?;
        self.load_accounts()?;
        self.status = Some("Account removed".to_string());
        Ok(())
    }

    fn execute_rename_account(&mut self, account_id: &str, nickname: Option<&str>) -> Result<()> {
        crate::commands::accounts::set_nickname_quiet(account_id, nickname, true)?;
        self.load_accounts()?;
        self.status = Some("Account renamed".to_string());
        Ok(())
    }

    fn execute_set_account_type(&mut self, account_id: &str, account_type: &str) -> Result<()> {
        crate::commands::accounts::set_type_quiet(account_id, account_type, true)?;
        self.load_accounts()?;
        self.status = Some("Account type updated".to_string());
        Ok(())
    }

    // Transaction filter actions

    pub fn show_filter_account_dialog(&mut self) {
        // Build list of accounts
        let items: Vec<(String, String)> = self.accounts
            .iter()
            .map(|a| (a.id.clone(), a.display_name().to_string()))
            .collect();

        if items.is_empty() {
            self.status = Some("No accounts to filter by".to_string());
            return;
        }

        self.show_dialog(
            DialogType::Select {
                title: "Filter by Account".to_string(),
                items,
                selected: 0,
            },
            DialogAction::FilterByAccount,
        );
    }

    pub fn clear_filters(&mut self) -> Result<()> {
        self.filter_account = None;
        self.search_query.clear();
        self.load_data()?;
        self.status = Some("Filters cleared".to_string());
        Ok(())
    }

    // Category actions (one-off)

    pub fn show_categorize_dialog(&mut self) {
        let selected = self.transactions_state.selected().unwrap_or(0);
        if let Some(tx) = self.transactions.get(selected) {
            let tx_id = tx.id.clone();
            let current_category = tx.category.clone();

            let categories = match Category::find_all_for_select_strings(&self.conn) {
                Ok(cats) => cats,
                Err(_) => {
                    self.status = Some("Failed to load categories".to_string());
                    return;
                }
            };

            if categories.is_empty() {
                self.status = Some("No categories defined. Run 'pint setup' first.".to_string());
                return;
            }

            // Find current category index
            let selected_idx = current_category
                .as_ref()
                .and_then(|cat| categories.iter().position(|(_, name)| name == cat))
                .unwrap_or(0);

            self.show_dialog(
                DialogType::Select {
                    title: "Set Category".to_string(),
                    items: categories,
                    selected: selected_idx,
                },
                DialogAction::CategorizeTransaction { tx_id },
            );
        }
    }

    fn execute_categorize_transaction(&mut self, tx_id: &str, category_id: i64, category_name: &str) -> Result<()> {
        Transaction::set_category(&self.conn, tx_id, category_id)?;

        self.load_transactions()?;
        self.status = Some(format!("Category set to '{}'", category_name));
        Ok(())
    }

    // Rule actions

    pub fn show_rule_dialog(&mut self) {
        let selected = self.transactions_state.selected().unwrap_or(0);
        if let Some(tx) = self.transactions.get(selected) {
            let tx_id = tx.id.clone();
            let description = tx.description.clone();
            let category = tx.category.clone();

            let categories = match Category::find_all_for_select(&self.conn) {
                Ok(cats) => cats,
                Err(_) => {
                    self.status = Some("Failed to load categories".to_string());
                    return;
                }
            };

            if categories.is_empty() {
                self.status = Some("No categories defined. Run 'pint setup' first.".to_string());
                return;
            }

            // Also load rules for finding existing rules
            let _ = self.load_rules();

            // Check if there's an existing rule for this transaction's category
            let (action, title, pattern, match_mode, selected_category) = if let Some(ref cat_name) = category {
                // Find if there's a rule that matches this transaction
                if let Some(rule) = self.find_rule_for_transaction(&description) {
                    // Edit existing rule
                    let match_mode = if rule.match_mode == "token" { 1 } else { 0 };
                    let selected_cat = categories.iter().position(|(_, name)| name == cat_name).unwrap_or(0);
                    (
                        DialogAction::EditRule { tx_id, rule_pattern: rule.pattern.clone() },
                        "Edit Rule".to_string(),
                        rule.pattern.clone(),
                        match_mode,
                        selected_cat,
                    )
                } else {
                    // Create new rule (transaction categorized manually perhaps)
                    let selected_cat = categories.iter().position(|(_, name)| name == cat_name).unwrap_or(0);
                    (
                        DialogAction::CreateRule { tx_id },
                        "Create Rule".to_string(),
                        description.clone(),
                        0,
                        selected_cat,
                    )
                }
            } else {
                // Uncategorized - create new rule
                (
                    DialogAction::CreateRule { tx_id },
                    "Create Rule".to_string(),
                    description.clone(),
                    0,
                    0,
                )
            };

            let cursor1 = pattern.len();
            self.dialog = Some(Dialog {
                dialog_type: DialogType::RuleEditor {
                    title,
                    focused_field: 0,
                    match_mode,
                    categories,
                    selected_category,
                },
                input1: pattern,
                input2: String::new(),
                action,
                cursor1,
                cursor2: 0,
            });
        }
    }

    fn find_rule_for_transaction(&self, description: &str) -> Option<&RuleRow> {
        let desc_lower = description.to_lowercase();
        self.rules.iter().find(|rule| {
            let pattern_lower = rule.pattern.to_lowercase();
            if rule.match_mode == "token" {
                // Token match: check if any word starts with the pattern
                desc_lower.split_whitespace().any(|word| word.starts_with(&pattern_lower))
            } else {
                // Substring match
                desc_lower.contains(&pattern_lower)
            }
        })
    }

    fn execute_create_rule(&mut self, _tx_id: &str, pattern: &str, match_mode: &str, category_id: i64, category_name: &str) -> Result<()> {
        MerchantRule::upsert(&self.conn, pattern, match_mode, category_id)?;
        let count = MerchantRule::apply_to_transactions(&self.conn, pattern, match_mode, category_id)?;

        // Reload data
        self.load_rules()?;
        self.load_transactions()?;

        self.status = Some(format!("Rule created: '{}' → {} ({} transactions)", pattern, category_name, count));
        Ok(())
    }

    fn execute_edit_rule(&mut self, _tx_id: &str, old_pattern: &str, new_pattern: &str, match_mode: &str, category_id: i64, category_name: &str) -> Result<()> {
        MerchantRule::delete_by_pattern(&self.conn, old_pattern)?;
        MerchantRule::upsert(&self.conn, new_pattern, match_mode, category_id)?;
        let count = MerchantRule::apply_to_transactions(&self.conn, new_pattern, match_mode, category_id)?;

        // Reload data
        self.load_rules()?;
        self.load_transactions()?;

        self.status = Some(format!("Rule updated: '{}' → {} ({} transactions)", new_pattern, category_name, count));
        Ok(())
    }

    // Sync actions

    /// Show sync dialog and set pending flag
    pub fn start_sync(&mut self) {
        self.dialog = Some(Dialog {
            dialog_type: DialogType::Info {
                title: "Sync".to_string(),
                message: "Syncing accounts...".to_string(),
            },
            input1: String::new(),
            input2: String::new(),
            action: DialogAction::AddAccount, // unused for Info dialog
            cursor1: 0,
            cursor2: 0,
        });
        self.pending_sync = true;
    }

    /// Execute sync operation (called after dialog is rendered)
    pub fn execute_sync(&mut self) {
        use crate::commands::sync;

        self.pending_sync = false;

        match sync::run_with_conn(&self.conn, 30) {
            Ok(stats) => {
                self.close_dialog();
                self.status = Some(stats.summary());
                let _ = self.load_accounts();
            }
            Err(e) => {
                self.close_dialog();
                self.status = Some(format!("Sync failed: {}", e));
            }
        }
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
