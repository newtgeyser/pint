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

    let left_part = format!(" 🍺 PINT - {}{}", app.current_view.name(),
        if account_filter.is_empty() { String::new() } else { format!(" {}", account_filter) });

    let right_part = match app.current_view {
        View::Summary => {
            format!("💰 Net Worth: ${} ", format_amount(app.summary.net_worth, "USD"))
        }
        View::Accounts => {
            let total = app.accounts_total();
            format!("💰 Total: ${} ", format_amount(total, "USD"))
        }
        View::Transactions => {
            format!("{} transactions ", app.transactions.len())
        }
        View::Holdings => {
            let total = app.holdings_total();
            format!("{} holdings  💰 Total: ${} ", app.holdings.len(), format_amount(total, "USD"))
        }
        View::Assets => {
            let total = app.assets_total();
            format!("💰 Total: ${} ", format_amount(total, "USD"))
        }
        View::Recurring => {
            format!("{} recurring ", app.recurring.len())
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
            Constraint::Length(20),  // Navigation (wider for emojis)
            Constraint::Min(0),      // Main content
        ])
        .split(area);

    draw_navigation(frame, app, chunks[0]);
    draw_main_content(frame, app, chunks[1]);
}

fn draw_navigation(frame: &mut Frame, app: &App, area: Rect) {
    let mut items: Vec<ListItem> = vec![ListItem::new("")]; // Empty line for spacing

    items.extend(View::ALL
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
            ListItem::new(format!("{}{}", prefix, view.label())).style(style)
        }));

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
        View::Summary => draw_summary(frame, app, inner),
        View::Accounts => draw_accounts(frame, app, inner),
        View::Transactions => draw_transactions(frame, app, inner),
        View::Holdings => draw_holdings(frame, app, inner),
        View::Assets => draw_assets(frame, app, inner),
        View::Recurring => draw_recurring(frame, app, inner),
        View::Rules => draw_rules(frame, app, inner),
    }
}

fn draw_summary(frame: &mut Frame, app: &App, area: Rect) {
    let summary = &app.summary;

    // Helper to format a row with label and value
    let format_row = |label: &str, value: i64, color: Color| -> Line {
        let amount_str = format!("${}", format_amount(value, "USD"));
        Line::from(vec![
            Span::styled(format!("{:<20}", label), Style::default().fg(Color::Gray)),
            Span::styled(format!("{:>15}", amount_str), Style::default().fg(color)),
        ])
    };

    // Determine colors based on positive/negative
    let positive_color = Color::Green;
    let negative_color = Color::Red;

    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled("  Net Worth Summary", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled("  ─────────────────────────────────", Style::default().fg(Color::DarkGray))),
        format_row("  Cash", summary.cash, if summary.cash >= 0 { positive_color } else { negative_color }),
        format_row("  Brokerage", summary.brokerage, if summary.brokerage >= 0 { positive_color } else { negative_color }),
        format_row("  Retirement", summary.retirement, if summary.retirement >= 0 { positive_color } else { negative_color }),
        format_row("  Assets", summary.assets, if summary.assets >= 0 { positive_color } else { negative_color }),
        format_row("  Credit Cards", summary.credit, if summary.credit >= 0 { positive_color } else { negative_color }),
        Line::from(Span::styled("  ─────────────────────────────────", Style::default().fg(Color::DarkGray))),
        Line::from(""),
        Line::from(vec![
            Span::styled(format!("{:<20}", "  NET WORTH"), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("{:>15}", format!("${}", format_amount(summary.net_worth, "USD"))),
                Style::default()
                    .fg(if summary.net_worth >= 0 { Color::Green } else { Color::Red })
                    .add_modifier(Modifier::BOLD)
            ),
        ]),
    ];

    // Add spending by category section if there are any
    if !summary.category_spending.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("  Spending by Category (Last 90 Days)", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))));
        lines.push(Line::from(Span::styled("  ──────────────────────────────────────────", Style::default().fg(Color::DarkGray))));

        for (category, amount) in &summary.category_spending {
            let amount_str = format!("${}", format_amount(*amount, "USD"));
            let category_display = if category.len() > 28 {
                format!("{}...", &category[..25])
            } else {
                category.clone()
            };

            lines.push(Line::from(vec![
                Span::styled(format!("  {:<28}", category_display), Style::default().fg(Color::Gray)),
                Span::styled(format!("{:>12}", amount_str), Style::default().fg(Color::Red)),
            ]));
        }
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
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

            // Reimbursement badge: green "[$:name]" if paid, yellow "[E:name]" if pending
            let mut desc_spans: Vec<Span> = Vec::new();
            let reserved_for_badge = match tx.reimburser.as_deref() {
                Some(name) => {
                    let (badge_text, badge_style) = if tx.reimbursed_at.is_some() {
                        (format!("[$:{}] ", truncate(name, 12)), Style::default().fg(Color::Green))
                    } else {
                        (format!("[E:{}] ", truncate(name, 12)), Style::default().fg(Color::Yellow))
                    };
                    let len = badge_text.chars().count();
                    desc_spans.push(Span::styled(badge_text, badge_style));
                    len
                }
                None => 0,
            };
            let remaining = desc_width.saturating_sub(2).saturating_sub(reserved_for_badge);
            desc_spans.push(Span::raw(truncate(&desc, remaining).to_string()));

            Row::new(vec![
                Line::from(Span::raw(tx.date.clone())),
                Line::from(Span::styled(amount_str, amount_style)),
                Line::from(Span::raw(truncate(cat_str, 16).to_string())),
                Line::from(desc_spans),
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

fn draw_recurring(frame: &mut Frame, app: &mut App, area: Rect) {
    if app.recurring.is_empty() {
        let msg = Paragraph::new("No recurring transactions detected.\n\nRecurring transactions are identified by:\n  - At least 3 similar transactions\n  - Consistent intervals (monthly or bi-monthly)");
        frame.render_widget(msg, area);
        return;
    }

    // Fixed columns: AMOUNT(12) + FREQ(12) + COUNT(6) + LAST(12) = 42
    // MERCHANT gets the remaining space
    let fixed_width: u16 = 12 + 12 + 6 + 12;
    let merchant_width = area.width.saturating_sub(fixed_width + 4) as usize; // +4 for column spacing

    let header = Row::new(vec!["MERCHANT", "AMOUNT", "FREQUENCY", "COUNT", "LAST"])
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .bottom_margin(1);

    let rows: Vec<Row> = app
        .recurring
        .iter()
        .map(|r| {
            let amount_str = format!("{:.2}", r.avg_amount);
            let amount_style = if r.avg_amount < 0.0 {
                Style::default().fg(Color::Red)
            } else {
                Style::default().fg(Color::Green)
            };

            let category_suffix = r.category
                .as_ref()
                .map(|c| format!(" [{}]", truncate(c, 12)))
                .unwrap_or_default();

            Row::new(vec![
                Span::raw(format!("{}{}", truncate(&r.merchant, merchant_width.saturating_sub(15)), category_suffix)),
                Span::styled(format!("{:>12}", amount_str), amount_style),
                Span::raw(format!("{:>12}", r.frequency)),
                Span::raw(format!("{:>6}", r.occurrences)),
                Span::raw(r.last_date.clone()),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Min(15),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(6),
            Constraint::Length(12),
        ],
    )
    .header(header)
    .row_highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White))
    .highlight_spacing(HighlightSpacing::Always);

    frame.render_stateful_widget(table, area, &mut app.recurring_state);
}

fn draw_rules(frame: &mut Frame, app: &mut App, area: Rect) {
    // Show filter info if active
    let filter_info = if !app.search_query.is_empty() {
        format!(" [Search: {}] ", app.search_query)
    } else {
        String::new()
    };

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
        let msg = if filter_info.is_empty() {
            "No rules found. Use 'pint rules import' to load rules.".to_string()
        } else {
            format!("No rules matching '{}' found.", app.search_query)
        };
        let msg = Paragraph::new(msg);
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
            View::Summary => {
                "s:Sync  Tab:Switch pane  q:Quit".to_string()
            }
            View::Accounts => {
                "s:Sync  a:Add  d:Remove  r:Rename  t:Type  Enter:View  /:Search  q:Quit".to_string()
            }
            View::Transactions => {
                let filter_status = if app.filter_account.is_some() || !app.search_query.is_empty() {
                    "  c:Clear"
                } else {
                    ""
                };
                format!("e:Edit category  l:Learn rule  r:Reimbursable  p:Toggle paid  /:Search  f:Filter{}  q:Quit", filter_status)
            }
            View::Holdings => {
                "e:Edit  q:Quit".to_string()
            }
            View::Assets => {
                "a:Add  d:Remove  e:Edit  q:Quit".to_string()
            }
            View::Recurring => {
                "↑↓:Navigate  q:Quit".to_string()
            }
            View::Rules => {
                let filter_status = if !app.search_query.is_empty() { "  c:Clear" } else { "" };
                format!("e:Edit  a:Apply rules  /:Search{}  q:Quit", filter_status)
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
        DialogType::Info { .. } => 5,
        DialogType::Confirm { .. } => 7,
        DialogType::Input { .. } => 7,
        DialogType::TwoInputs { .. } => 10,
        DialogType::Select { items, .. } => {
            // Filter items based on search input
            let filter = dialog.input1.to_lowercase();
            let filtered_count = if filter.is_empty() {
                items.len()
            } else {
                items.iter().filter(|(_, name)| name.to_lowercase().contains(&filter)).count()
            };
            // Height: +5 for border, search box, gap, and instructions
            let item_count = filtered_count as u16;
            (item_count + 5).clamp(8, area.height.saturating_sub(6))
        }
        DialogType::RuleEditor { .. } => 18, // Fixed height for rule editor
        DialogType::AssetEditor { .. } => 16, // Fixed height for asset editor
        DialogType::HoldingEditor { .. } => 14, // Fixed height for holding editor
    };

    let dialog_x = (area.width.saturating_sub(dialog_width)) / 2;
    let dialog_y = (area.height.saturating_sub(dialog_height)) / 2;

    let dialog_area = Rect::new(dialog_x, dialog_y, dialog_width, dialog_height);

    // Clear the area behind the dialog and fill with background
    frame.render_widget(Clear, dialog_area);
    let bg_block = Block::default().style(Style::default().bg(Color::Black));
    frame.render_widget(bg_block, dialog_area);

    // Draw dialog content based on type
    match &dialog.dialog_type {
        DialogType::Info { title, message } => {
            let block = Block::default()
                .title(format!(" {} ", title))
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Cyan).bg(Color::Black));

            let inner = block.inner(dialog_area);
            frame.render_widget(block, dialog_area);

            let text = vec![
                Line::from(""),
                Line::from(message.as_str()),
            ];
            let paragraph = Paragraph::new(text)
                .style(Style::default().bg(Color::Black))
                .alignment(ratatui::layout::Alignment::Center);
            frame.render_widget(paragraph, inner);
        }
        DialogType::Confirm { title, message } => {
            let block = Block::default()
                .title(format!(" {} ", title))
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Yellow).bg(Color::Black));

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
                .style(Style::default().fg(Color::Cyan).bg(Color::Black));

            let inner = block.inner(dialog_area);
            frame.render_widget(block, dialog_area);

            // Split input at cursor position
            let (before, after) = dialog.input1.split_at(dialog.cursor1.min(dialog.input1.len()));
            let input_style = Style::default().fg(Color::White).add_modifier(Modifier::BOLD);

            let text = vec![
                Line::from(Span::raw(prompt.clone())),
                Line::from(""),
                Line::from(vec![
                    Span::styled(before.to_string(), input_style),
                    Span::styled("█", input_style),
                    Span::styled(after.to_string(), input_style),
                ]),
            ];

            let paragraph = Paragraph::new(text);
            frame.render_widget(paragraph, inner);
        }

        DialogType::TwoInputs { title, prompt1, prompt2, focused } => {
            let block = Block::default()
                .title(format!(" {} ", title))
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Cyan).bg(Color::Black));

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

            // Build input1 line with cursor
            let input1_line = if *focused == 0 {
                let (before, after) = dialog.input1.split_at(dialog.cursor1.min(dialog.input1.len()));
                Line::from(vec![
                    Span::styled(before.to_string(), input1_style),
                    Span::styled("█", input1_style),
                    Span::styled(after.to_string(), input1_style),
                ])
            } else {
                Line::from(Span::styled(dialog.input1.clone(), input1_style))
            };

            // Build input2 line with cursor
            let input2_line = if *focused == 1 {
                let (before, after) = dialog.input2.split_at(dialog.cursor2.min(dialog.input2.len()));
                Line::from(vec![
                    Span::styled(before.to_string(), input2_style),
                    Span::styled("█", input2_style),
                    Span::styled(after.to_string(), input2_style),
                ])
            } else {
                Line::from(Span::styled(dialog.input2.clone(), input2_style))
            };

            let text = vec![
                Line::from(Span::raw(prompt1.clone())),
                input1_line,
                Line::from(""),
                Line::from(Span::raw(prompt2.clone())),
                input2_line,
                Line::from(""),
                Line::from(Span::styled("Tab to switch fields", Style::default().fg(Color::DarkGray))),
            ];

            let paragraph = Paragraph::new(text);
            frame.render_widget(paragraph, inner);
        }

        DialogType::Select { title, items, selected } => {
            let block = Block::default()
                .title(format!(" {} ", title))
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Cyan).bg(Color::Black));

            let inner = block.inner(dialog_area);
            frame.render_widget(block, dialog_area);

            // Split inner area: search box at top, list below
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1), // Search input
                    Constraint::Length(1), // Gap
                    Constraint::Min(1),    // List
                ])
                .split(inner);

            // Render search input with cursor
            let filter = &dialog.input1;
            let search_text = format!("🔍 {}_", filter);
            let search_line = Line::from(vec![
                Span::styled(&search_text[..4], Style::default().fg(Color::DarkGray)), // magnifier
                Span::styled(&filter[..], Style::default().fg(Color::White)),
                Span::styled("▏", Style::default().fg(Color::Yellow)), // cursor
            ]);
            frame.render_widget(Paragraph::new(search_line), chunks[0]);

            // Filter items based on search input
            let filter_lower = filter.to_lowercase();
            let filtered_items: Vec<(usize, &(String, String))> = items
                .iter()
                .enumerate()
                .filter(|(_, (_, name))| {
                    filter_lower.is_empty() || name.to_lowercase().contains(&filter_lower)
                })
                .collect();

            // Build list items from filtered results
            let list_items: Vec<ListItem> = filtered_items
                .iter()
                .enumerate()
                .map(|(i, (_, (_, display_name)))| {
                    let style = if i == *selected {
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    let prefix = if i == *selected { "▸ " } else { "  " };
                    ListItem::new(format!("{}{}", prefix, truncate(display_name, 50))).style(style)
                })
                .collect();

            let list = List::new(list_items);
            frame.render_widget(list, chunks[2]);
        }

        DialogType::RuleEditor { title, focused_field, match_mode, categories, selected_category } => {
            let block = Block::default()
                .title(format!(" {} ", title))
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Cyan).bg(Color::Black));

            let inner = block.inner(dialog_area);
            frame.render_widget(block, dialog_area);

            // Styles for focused/unfocused fields
            let focused_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
            let unfocused_style = Style::default().fg(Color::Gray);
            let label_style = Style::default().fg(Color::White);

            // Pattern field
            let pattern_style = if *focused_field == 0 { focused_style } else { unfocused_style };
            let (before, after) = dialog.input1.split_at(dialog.cursor1.min(dialog.input1.len()));
            let pattern_line = if *focused_field == 0 {
                Line::from(vec![
                    Span::styled(before.to_string(), pattern_style),
                    Span::styled("█", pattern_style),
                    Span::styled(after.to_string(), pattern_style),
                ])
            } else {
                Line::from(Span::styled(dialog.input1.clone(), pattern_style))
            };

            // Match mode field
            let match_style = if *focused_field == 1 { focused_style } else { unfocused_style };
            let substring_prefix = if *match_mode == 0 { "● " } else { "○ " };
            let token_prefix = if *match_mode == 1 { "● " } else { "○ " };

            // Category field - show a few categories around the selected one
            let cat_style = if *focused_field == 2 { focused_style } else { unfocused_style };

            // Build the dialog content
            let mut lines = vec![
                Line::from(Span::styled("Pattern:", label_style)),
                pattern_line,
                Line::from(""),
                Line::from(Span::styled("Match mode:", label_style)),
                Line::from(Span::styled(format!("{}Substring", substring_prefix), match_style)),
                Line::from(Span::styled(format!("{}Token", token_prefix), match_style)),
                Line::from(""),
                Line::from(Span::styled("Category:", label_style)),
            ];

            // Show categories with selection indicator
            let start = selected_category.saturating_sub(2);
            let visible_cats: Vec<_> = categories.iter().enumerate().skip(start).take(5).collect();
            for (i, (_, name)) in visible_cats {
                let prefix = if i == *selected_category { "▸ " } else { "  " };
                let style = if i == *selected_category { cat_style } else { unfocused_style };
                lines.push(Line::from(Span::styled(format!("{}{}", prefix, truncate(name, 45)), style)));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled("Tab:Next field  ↑↓:Select  Enter:Confirm", Style::default().fg(Color::DarkGray))));

            let paragraph = Paragraph::new(lines);
            frame.render_widget(paragraph, inner);
        }

        DialogType::AssetEditor { title, focused_field, selected_type } => {
            let block = Block::default()
                .title(format!(" {} ", title))
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Cyan).bg(Color::Black));

            let inner = block.inner(dialog_area);
            frame.render_widget(block, dialog_area);

            // Styles for focused/unfocused fields
            let focused_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
            let unfocused_style = Style::default().fg(Color::Gray);
            let label_style = Style::default().fg(Color::White);

            // Name field (input1)
            let name_style = if *focused_field == 0 { focused_style } else { unfocused_style };
            let (before, after) = dialog.input1.split_at(dialog.cursor1.min(dialog.input1.len()));
            let name_line = if *focused_field == 0 {
                Line::from(vec![
                    Span::styled(before.to_string(), name_style),
                    Span::styled("█", name_style),
                    Span::styled(after.to_string(), name_style),
                ])
            } else {
                Line::from(Span::styled(dialog.input1.clone(), name_style))
            };

            // Type field (selection)
            let type_style = if *focused_field == 1 { focused_style } else { unfocused_style };
            let asset_types = ["Real Estate", "Vehicle", "Collectible", "Other"];

            // Value field (input2)
            let value_style = if *focused_field == 2 { focused_style } else { unfocused_style };
            let (before2, after2) = dialog.input2.split_at(dialog.cursor2.min(dialog.input2.len()));
            let value_line = if *focused_field == 2 {
                Line::from(vec![
                    Span::styled("$", value_style),
                    Span::styled(before2.to_string(), value_style),
                    Span::styled("█", value_style),
                    Span::styled(after2.to_string(), value_style),
                ])
            } else {
                Line::from(vec![
                    Span::styled("$", value_style),
                    Span::styled(dialog.input2.clone(), value_style),
                ])
            };

            // Build the dialog content
            let mut lines = vec![
                Line::from(Span::styled("Name:", label_style)),
                name_line,
                Line::from(""),
                Line::from(Span::styled("Type:", label_style)),
            ];

            // Show asset types with selection indicator
            for (i, type_name) in asset_types.iter().enumerate() {
                let prefix = if i == *selected_type { "● " } else { "○ " };
                let style = if i == *selected_type { type_style } else { unfocused_style };
                lines.push(Line::from(Span::styled(format!("{}{}", prefix, type_name), style)));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled("Value:", label_style)));
            lines.push(value_line);
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled("Tab:Next field  ↑↓:Select  Enter:Confirm", Style::default().fg(Color::DarkGray))));

            let paragraph = Paragraph::new(lines);
            frame.render_widget(paragraph, inner);
        }

        DialogType::HoldingEditor { title, focused_field, .. } => {
            let block = Block::default()
                .title(format!(" {} ", title))
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Cyan).bg(Color::Black));

            let inner = block.inner(dialog_area);
            frame.render_widget(block, dialog_area);

            // Styles for focused/unfocused fields
            let focused_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
            let unfocused_style = Style::default().fg(Color::Gray);
            let label_style = Style::default().fg(Color::White);

            // Symbol field (input1)
            let symbol_style = if *focused_field == 0 { focused_style } else { unfocused_style };
            let (before1, after1) = dialog.input1.split_at(dialog.cursor1.min(dialog.input1.len()));
            let symbol_line = if *focused_field == 0 {
                Line::from(vec![
                    Span::styled(before1.to_string(), symbol_style),
                    Span::styled("█", symbol_style),
                    Span::styled(after1.to_string(), symbol_style),
                ])
            } else {
                Line::from(Span::styled(dialog.input1.clone(), symbol_style))
            };

            // Shares field (input2)
            let shares_style = if *focused_field == 1 { focused_style } else { unfocused_style };
            let (before2, after2) = dialog.input2.split_at(dialog.cursor2.min(dialog.input2.len()));
            let shares_line = if *focused_field == 1 {
                Line::from(vec![
                    Span::styled(before2.to_string(), shares_style),
                    Span::styled("█", shares_style),
                    Span::styled(after2.to_string(), shares_style),
                ])
            } else {
                Line::from(Span::styled(dialog.input2.clone(), shares_style))
            };

            // Price field (input3)
            let price_style = if *focused_field == 2 { focused_style } else { unfocused_style };
            let (before3, after3) = dialog.input3.split_at(dialog.cursor3.min(dialog.input3.len()));
            let price_line = if *focused_field == 2 {
                Line::from(vec![
                    Span::styled("$", price_style),
                    Span::styled(before3.to_string(), price_style),
                    Span::styled("█", price_style),
                    Span::styled(after3.to_string(), price_style),
                ])
            } else {
                Line::from(vec![
                    Span::styled("$", price_style),
                    Span::styled(dialog.input3.clone(), price_style),
                ])
            };

            let lines = vec![
                Line::from(Span::styled("Symbol (e.g., AAPL, BTC-USD):", label_style)),
                symbol_line,
                Line::from(""),
                Line::from(Span::styled("Shares:", label_style)),
                shares_line,
                Line::from(""),
                Line::from(Span::styled("Price:", label_style)),
                price_line,
                Line::from(""),
                Line::from(Span::styled("Tab:Next field  Enter:Confirm  Esc:Cancel", Style::default().fg(Color::DarkGray))),
            ];

            let paragraph = Paragraph::new(lines);
            frame.render_widget(paragraph, inner);
        }
    }
}
