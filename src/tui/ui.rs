use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Row, Table, HighlightSpacing},
    Frame,
};

use super::app::{format_amount, App, DialogType, View};
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

    // Draw dialog overlay if active
    if app.has_dialog() {
        draw_dialog(frame, app);
    }
}

fn draw_title(frame: &mut Frame, app: &App, area: Rect) {
    let account_filter = app.filter_account.as_ref()
        .map(|acc| format!("[{}]", truncate(acc, 20)))
        .unwrap_or_default();

    let left_part = format!(" PINT - {}{}", app.current_view.name(),
        if account_filter.is_empty() { String::new() } else { format!(" {}", account_filter) });

    let right_part = match app.current_view {
        View::Accounts => {
            let total = app.accounts_total();
            format!("Total: ${} ", format_amount(total, "USD"))
        }
        View::Transactions => {
            format!("{} transactions ", app.transactions.len())
        }
        View::Holdings => {
            let total = app.holdings_total();
            format!("{} holdings  Total: ${} ", app.holdings.len(), format_amount(total, "USD"))
        }
        View::Assets => {
            let total = app.assets_total();
            format!("Total: ${} ", format_amount(total, "USD"))
        }
        View::Rules => {
            format!("{} rules ", app.rules.len())
        }
    };

    // Calculate padding to right-justify the right part
    // area.width includes borders (2 chars)
    let available_width = area.width.saturating_sub(2) as usize;
    let left_len = left_part.chars().count();
    let right_len = right_part.chars().count();
    let padding = available_width.saturating_sub(left_len + right_len);

    let title = format!("{}{:padding$}{}", left_part, "", right_part, padding = padding);

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

    // Fixed columns: TYPE(14) + BALANCE(12) + CUR(4) = 30
    // ACCOUNT gets the remaining space
    let fixed_width: u16 = 14 + 12 + 4;
    let account_width = area.width.saturating_sub(fixed_width + 3) as usize; // +3 for column spacing

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
                truncate(account.display_name(), account_width.saturating_sub(2)).to_string(),
                format!("{:>12}", balance_str),
                account.currency.clone(),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(14),
            Constraint::Min(10),
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

    // Fixed columns: DATE(12) + AMOUNT(12) + CATEGORY(18) = 42
    // DESCRIPTION gets the remaining space
    let fixed_width: u16 = 12 + 12 + 18;
    let desc_width = area.width.saturating_sub(fixed_width + 3) as usize; // +3 for column spacing

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
                Span::raw(truncate(&desc, desc_width.saturating_sub(2)).to_string()),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(18),
            Constraint::Min(10),
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

    // Fixed columns: SYMBOL(10) + SHARES(12) + PRICE(10) + VALUE(12) + GAIN(10) = 54
    // DESCRIPTION gets the remaining space
    let fixed_width: u16 = 10 + 12 + 10 + 12 + 10;
    let desc_width = area.width.saturating_sub(fixed_width + 5) as usize; // +5 for column spacing

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
                truncate(desc, desc_width.saturating_sub(2)).to_string(),
                format!("{:>12}", truncate(&h.shares, 12)),
                format!("{:>10}", price_str),
                format!("{:>12}", value_str),
                format!("{:>10}", gain_str),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(10),
            Constraint::Min(10),
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

    // Fixed columns: ID(4) + TYPE(14) + VALUE(12) + COST(12) + GAIN(10) = 52
    // NAME gets the remaining space
    let fixed_width: u16 = 4 + 14 + 12 + 12 + 10;
    let name_width = area.width.saturating_sub(fixed_width + 5) as usize; // +5 for column spacing

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
                truncate(&a.name, name_width.saturating_sub(2)).to_string(),
                format!("{:>12}", value_str),
                format!("{:>12}", cost_str),
                format!("{:>10}", gain_str),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(4),
            Constraint::Length(14),
            Constraint::Min(10),
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

    // Fixed columns: MATCH(12) + CATEGORY(22) = 34
    // PATTERN gets the remaining space
    let fixed_width: u16 = 12 + 22;
    let pattern_width = chunks[1].width.saturating_sub(fixed_width + 2) as usize; // +2 for column spacing

    let header = Row::new(vec!["PATTERN", "MATCH", "CATEGORY"])
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .bottom_margin(1);

    let rows: Vec<Row> = app
        .rules
        .iter()
        .map(|r| {
            Row::new(vec![
                truncate(&r.pattern, pattern_width.saturating_sub(2)).to_string(),
                truncate(&r.match_mode, 10).to_string(),
                truncate(&r.category, 20).to_string(),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Min(10),
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

    let help_text = if app.has_dialog() {
        "Enter:Confirm  Esc:Cancel  Tab:Next field".to_string()
    } else if app.is_searching() {
        format!("Search: {}█  (Esc to cancel)", app.search_query)
    } else if let Some(ref msg) = app.status {
        msg.clone()
    } else if !app.has_interacted {
        // Show general help on first launch
        "q:Quit  Tab:Switch pane  ↑↓/jk:Navigate  PgUp/PgDn:Page  Enter:Select".to_string()
    } else {
        // Show view-specific commands after user interacts
        match app.current_view {
            View::Accounts => {
                "a:Add  d:Remove  r:Rename  t:Set type  Enter:View  /:Search  q:Quit".to_string()
            }
            View::Transactions => {
                let filter_status = if app.filter_account.is_some() || !app.search_query.is_empty() {
                    "  c:Clear filters"
                } else {
                    ""
                };
                format!("/:Search  f:Filter account{}  q:Quit", filter_status)
            }
            View::Holdings => {
                "Enter:View details  q:Quit".to_string()
            }
            View::Assets => {
                "a:Add  d:Remove  e:Edit  q:Quit".to_string()
            }
            View::Rules => {
                "i:Import  p:Apply rules  q:Quit".to_string()
            }
        }
    };

    let status = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Gray))
        .block(status_block);

    frame.render_widget(status, area);
}

fn draw_dialog(frame: &mut Frame, app: &App) {
    let dialog = match &app.dialog {
        Some(d) => d,
        None => return,
    };

    // Calculate dialog size and position (centered)
    let area = frame.area();
    let dialog_width = 60.min(area.width.saturating_sub(4));
    let dialog_height = match &dialog.dialog_type {
        DialogType::Confirm { .. } => 7,
        DialogType::Input { .. } => 7,
        DialogType::TwoInputs { .. } => 10,
    };

    let dialog_x = (area.width.saturating_sub(dialog_width)) / 2;
    let dialog_y = (area.height.saturating_sub(dialog_height)) / 2;

    let dialog_area = Rect::new(dialog_x, dialog_y, dialog_width, dialog_height);

    // Clear the area behind the dialog
    frame.render_widget(Clear, dialog_area);

    // Draw dialog content based on type
    match &dialog.dialog_type {
        DialogType::Confirm { title, message } => {
            let block = Block::default()
                .title(format!(" {} ", title))
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Yellow));

            let inner = block.inner(dialog_area);
            frame.render_widget(block, dialog_area);

            let text = vec![
                Line::from(""),
                Line::from(Span::raw(message.clone())),
                Line::from(""),
                Line::from(Span::styled(
                    "Enter to confirm, Esc to cancel",
                    Style::default().fg(Color::Gray),
                )),
            ];

            let paragraph = Paragraph::new(text);
            frame.render_widget(paragraph, inner);
        }

        DialogType::Input { title, prompt } => {
            let block = Block::default()
                .title(format!(" {} ", title))
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Cyan));

            let inner = block.inner(dialog_area);
            frame.render_widget(block, dialog_area);

            let text = vec![
                Line::from(Span::raw(prompt.clone())),
                Line::from(""),
                Line::from(Span::styled(
                    format!("{}█", dialog.input1),
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                )),
            ];

            let paragraph = Paragraph::new(text);
            frame.render_widget(paragraph, inner);
        }

        DialogType::TwoInputs { title, prompt1, prompt2, focused } => {
            let block = Block::default()
                .title(format!(" {} ", title))
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Cyan));

            let inner = block.inner(dialog_area);
            frame.render_widget(block, dialog_area);

            let input1_style = if *focused == 0 {
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };

            let input2_style = if *focused == 1 {
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };

            let cursor1 = if *focused == 0 { "█" } else { "" };
            let cursor2 = if *focused == 1 { "█" } else { "" };

            let text = vec![
                Line::from(Span::raw(prompt1.clone())),
                Line::from(Span::styled(format!("{}{}", dialog.input1, cursor1), input1_style)),
                Line::from(""),
                Line::from(Span::raw(prompt2.clone())),
                Line::from(Span::styled(format!("{}{}", dialog.input2, cursor2), input2_style)),
                Line::from(""),
                Line::from(Span::styled("Tab to switch fields", Style::default().fg(Color::DarkGray))),
            ];

            let paragraph = Paragraph::new(text);
            frame.render_widget(paragraph, inner);
        }
    }
}
