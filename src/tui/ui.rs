use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, List, ListItem, Paragraph, Row, Table, HighlightSpacing},
    Frame,
};

use super::app::{format_amount, App, View};
use crate::util::truncate;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Title
            Constraint::Min(0),     // Content
            Constraint::Length(3),  // Status bar
        ])
        .split(frame.area());

    draw_title(frame, app, chunks[0]);
    draw_content(frame, app, chunks[1]);
    draw_status_bar(frame, app, chunks[2]);
}

fn draw_title(frame: &mut Frame, app: &App, area: Rect) {
    let detail = match app.current_view {
        View::Accounts => {
            let total = app.accounts_total();
            format!("  Total: ${}", format_amount(total, "USD"))
        }
        View::Transactions => {
            format!("  {} transactions", app.transactions.len())
        }
        View::Holdings => {
            let total = app.holdings_total();
            format!("  {} holdings  Total: ${}", app.holdings.len(), format_amount(total, "USD"))
        }
        View::Assets => {
            let total = app.assets_total();
            format!("  Total: ${}", format_amount(total, "USD"))
        }
        View::Rules => {
            format!("  {} rules", app.rules.len())
        }
    };

    let title = format!(" PINT - {}{} ", app.current_view.name(), detail);
    let title_block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::Cyan));

    let title_text = Paragraph::new(title)
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(title_block);

    frame.render_widget(title_text, area);
}

fn draw_content(frame: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(16),  // Navigation
            Constraint::Min(0),      // Main content
        ])
        .split(area);

    draw_navigation(frame, app, chunks[0]);
    draw_main_content(frame, app, chunks[1]);
}

fn draw_navigation(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = View::ALL
        .iter()
        .enumerate()
        .map(|(i, view)| {
            let style = if i == app.nav_index {
                if app.nav_focused {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                }
            } else {
                Style::default().fg(Color::Gray)
            };

            let prefix = if i == app.nav_index { "▸ " } else { "  " };
            ListItem::new(format!("{}{}", prefix, view.name())).style(style)
        })
        .collect();

    let nav_block = Block::default()
        .borders(Borders::ALL)
        .border_style(if app.nav_focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        })
        .title(" Navigation ");

    let nav_list = List::new(items).block(nav_block);

    frame.render_widget(nav_list, area);
}

fn draw_main_content(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(if !app.nav_focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        })
        .title(format!(" {} ", app.current_view.name()));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    match app.current_view {
        View::Accounts => draw_accounts(frame, app, inner),
        View::Transactions => draw_transactions(frame, app, inner),
        View::Holdings => draw_holdings(frame, app, inner),
        View::Assets => draw_assets(frame, app, inner),
        View::Rules => draw_rules(frame, app, inner),
    }
}

fn draw_accounts(frame: &mut Frame, app: &mut App, area: Rect) {
    if app.accounts.is_empty() {
        let msg = Paragraph::new("No accounts found. Run 'pint sync' to fetch accounts.");
        frame.render_widget(msg, area);
        return;
    }

    let header = Row::new(vec!["TYPE", "ACCOUNT", "BALANCE", "CUR"])
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .bottom_margin(1);

    let rows: Vec<Row> = app
        .accounts
        .iter()
        .map(|account| {
            let type_str = if account.manual {
                format!("{}*", account.account_type)
            } else {
                account.account_type.clone()
            };

            let balance_str = account
                .balance
                .map(|b| format_amount(b, &account.currency))
                .unwrap_or_else(|| "N/A".to_string());

            Row::new(vec![
                truncate(&type_str, 12).to_string(),
                truncate(account.display_name(), 24).to_string(),
                balance_str,
                account.currency.clone(),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(14),
            Constraint::Length(26),
            Constraint::Length(12),
            Constraint::Length(4),
        ],
    )
    .header(header)
    .row_highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White))
    .highlight_spacing(HighlightSpacing::Always);

    frame.render_stateful_widget(table, area, &mut app.accounts_state);
}

fn draw_transactions(frame: &mut Frame, app: &mut App, area: Rect) {
    // Show filter info if active
    let filter_info = if let Some(ref acc) = app.filter_account {
        format!(" [Account: {}] ", truncate(acc, 20))
    } else if !app.search_query.is_empty() {
        format!(" [Search: {}] ", app.search_query)
    } else {
        String::new()
    };

    if app.transactions.is_empty() {
        let msg = Paragraph::new(format!(
            "No transactions found.{}",
            if filter_info.is_empty() { "" } else { &filter_info }
        ));
        frame.render_widget(msg, area);
        return;
    }

    let header = Row::new(vec!["DATE", "AMOUNT", "CATEGORY", "DESCRIPTION"])
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .bottom_margin(1);

    let rows: Vec<Row> = app
        .transactions
        .iter()
        .map(|tx| {
            let amount_cents = (tx.amount * 100.0) as i64;
            let amount_str = format!("{:>12}", format_amount(amount_cents, "USD"));
            let amount_style = if tx.amount < 0.0 {
                Style::default().fg(Color::Red)
            } else {
                Style::default().fg(Color::Green)
            };

            let cat_str = tx.category.as_deref().unwrap_or("-");
            let desc = if tx.pending {
                format!("* {}", tx.description)
            } else {
                tx.description.clone()
            };

            Row::new(vec![
                Span::raw(tx.date.clone()),
                Span::styled(amount_str, amount_style),
                Span::raw(truncate(cat_str, 16).to_string()),
                Span::raw(truncate(&desc, 40).to_string()),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(18),
            Constraint::Min(20),
        ],
    )
    .header(header)
    .row_highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White))
    .highlight_spacing(HighlightSpacing::Always);

    frame.render_stateful_widget(table, area, &mut app.transactions_state);
}

fn draw_holdings(frame: &mut Frame, app: &mut App, area: Rect) {
    let filter_info = if let Some(ref acc) = app.filter_account {
        format!(" [Account: {}] ", truncate(acc, 20))
    } else {
        String::new()
    };

    if app.holdings.is_empty() {
        let msg = Paragraph::new(format!(
            "No holdings found.{}",
            if filter_info.is_empty() { "" } else { &filter_info }
        ));
        frame.render_widget(msg, area);
        return;
    }

    let header = Row::new(vec!["SYMBOL", "DESCRIPTION", "SHARES", "PRICE", "VALUE", "GAIN"])
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .bottom_margin(1);

    let rows: Vec<Row> = app
        .holdings
        .iter()
        .map(|h| {
            let symbol = h.symbol.as_deref().unwrap_or("-");
            let desc = h.description.as_deref().unwrap_or("-");

            let price_str = h.price
                .map(|p| format_amount(p, &h.currency))
                .unwrap_or_else(|| "N/A".to_string());

            let value_str = h.market_value
                .map(|v| format_amount(v, &h.currency))
                .unwrap_or_else(|| "N/A".to_string());

            let gain_str = match (h.cost_basis, h.market_value) {
                (Some(cost), Some(value)) if cost > 0 => {
                    let pct = ((value - cost) as f64 / cost as f64) * 100.0;
                    format!("{:+.1}%", pct)
                }
                _ => "N/A".to_string(),
            };

            Row::new(vec![
                truncate(symbol, 8).to_string(),
                truncate(desc, 20).to_string(),
                truncate(&h.shares, 10).to_string(),
                price_str,
                value_str,
                gain_str,
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(10),
            Constraint::Length(22),
            Constraint::Length(12),
            Constraint::Length(10),
            Constraint::Length(12),
            Constraint::Length(10),
        ],
    )
    .header(header)
    .row_highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White))
    .highlight_spacing(HighlightSpacing::Always);

    frame.render_stateful_widget(table, area, &mut app.holdings_state);
}

fn draw_assets(frame: &mut Frame, app: &mut App, area: Rect) {
    if app.assets.is_empty() {
        let msg = Paragraph::new("No assets found. Use 'pint assets add' to add assets.");
        frame.render_widget(msg, area);
        return;
    }

    let header = Row::new(vec!["ID", "TYPE", "NAME", "VALUE", "COST", "GAIN"])
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .bottom_margin(1);

    let rows: Vec<Row> = app
        .assets
        .iter()
        .map(|a| {
            let value_str = a.value
                .map(|v| format_amount(v, &a.currency))
                .unwrap_or_else(|| "N/A".to_string());

            let cost_str = a.cost_basis
                .map(|c| format_amount(c, &a.currency))
                .unwrap_or_else(|| "N/A".to_string());

            let gain_str = match (a.cost_basis, a.value) {
                (Some(cost), Some(value)) if cost > 0 => {
                    let pct = ((value - cost) as f64 / cost as f64) * 100.0;
                    format!("{:+.1}%", pct)
                }
                _ => "N/A".to_string(),
            };

            Row::new(vec![
                a.id.to_string(),
                truncate(&a.asset_type, 12).to_string(),
                truncate(&a.name, 24).to_string(),
                value_str,
                cost_str,
                gain_str,
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(4),
            Constraint::Length(14),
            Constraint::Length(26),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(10),
        ],
    )
    .header(header)
    .row_highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White))
    .highlight_spacing(HighlightSpacing::Always);

    frame.render_stateful_widget(table, area, &mut app.assets_state);
}

fn draw_rules(frame: &mut Frame, app: &mut App, area: Rect) {
    // Split area for categories and rules
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),  // Categories summary
            Constraint::Min(0),     // Rules list
        ])
        .split(area);

    // Categories summary
    let cat_count = app.categories.len();
    let cat_text = format!(
        "Categories: {} defined\n\nTop categories: {}",
        cat_count,
        app.categories.iter().take(5).map(|c| c.name.as_str()).collect::<Vec<_>>().join(", ")
    );
    let cat_para = Paragraph::new(cat_text)
        .style(Style::default().fg(Color::Gray));
    frame.render_widget(cat_para, chunks[0]);

    // Rules list
    if app.rules.is_empty() {
        let msg = Paragraph::new("No rules found. Use 'pint rules import' to load rules.");
        frame.render_widget(msg, chunks[1]);
        return;
    }

    let header = Row::new(vec!["PATTERN", "MATCH", "CATEGORY"])
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .bottom_margin(1);

    let rows: Vec<Row> = app
        .rules
        .iter()
        .map(|r| {
            Row::new(vec![
                truncate(&r.pattern, 30).to_string(),
                truncate(&r.match_mode, 10).to_string(),
                truncate(&r.category, 20).to_string(),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(32),
            Constraint::Length(12),
            Constraint::Length(22),
        ],
    )
    .header(header)
    .row_highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White))
    .highlight_spacing(HighlightSpacing::Always);

    frame.render_stateful_widget(table, chunks[1], &mut app.rules_state);
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let status_block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::DarkGray));

    let help_text = if app.is_searching() {
        format!("Search: {}█  (Esc to cancel)", app.search_query)
    } else if let Some(ref msg) = app.status {
        msg.clone()
    } else {
        let base = "q:Quit  Tab:Switch  ↑↓/jk:Navigate  PgUp/PgDn:Page  Enter:Select";
        let extra = match app.current_view {
            View::Transactions => "  /:Search",
            View::Accounts => "  Enter:View transactions",
            _ => "",
        };
        format!("{}{}", base, extra)
    };

    let status = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Gray))
        .block(status_block);

    frame.render_widget(status, area);
}
