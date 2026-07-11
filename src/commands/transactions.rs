use anyhow::{Context, Result};
use chrono::{NaiveDate, TimeZone, Utc};

use crate::db;
use crate::util::truncate;

pub struct Filters {
    pub account: Option<String>,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub search: Option<String>,
    pub category: Option<String>,
    pub uncategorized: bool,
    pub limit: Option<usize>,
}

struct TransactionRow {
    posted: i64,
    amount: i64,
    description: String,
    pending: bool,
    category_name: Option<String>,
}

pub fn run(filters: Filters) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint init' first.")?;

    let mut sql = String::from(
        "SELECT t.posted, t.amount, t.description, t.pending, c.name as category_name
         FROM transactions t
         JOIN accounts a ON t.account_id = a.id
         LEFT JOIN categories c ON t.category_id = c.id
         WHERE 1=1",
    );

    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(ref account) = filters.account {
        let param_num = params.len() + 1;
        sql.push_str(&format!(
            " AND (a.id = ?{p} OR a.id LIKE ?{p} || '%' OR a.name LIKE '%' || ?{p} || '%')",
            p = param_num
        ));
        params.push(Box::new(account.clone()));
    }

    if let Some(from) = filters.from {
        let ts = from.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp();
        let param_num = params.len() + 1;
        sql.push_str(&format!(" AND t.posted >= ?{}", param_num));
        params.push(Box::new(ts));
    }

    if let Some(to) = filters.to {
        let ts = to.and_hms_opt(23, 59, 59).unwrap().and_utc().timestamp();
        let param_num = params.len() + 1;
        sql.push_str(&format!(" AND t.posted <= ?{}", param_num));
        params.push(Box::new(ts));
    }

    if let Some(ref search) = filters.search {
        let param_num = params.len() + 1;
        sql.push_str(&format!(" AND t.description LIKE ?{}", param_num));
        params.push(Box::new(format!("%{}%", search)));
    }

    if let Some(ref category) = filters.category {
        let param_num = params.len() + 1;
        sql.push_str(&format!(" AND c.name LIKE ?{}", param_num));
        params.push(Box::new(format!("%{}%", category)));
    }

    if filters.uncategorized {
        sql.push_str(" AND t.category_id IS NULL");
    }

    sql.push_str(" ORDER BY t.posted DESC");

    if let Some(limit) = filters.limit {
        sql.push_str(&format!(" LIMIT {}", limit));
    }

    let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let mut stmt = conn.prepare(&sql)?;
    let transactions: Vec<TransactionRow> = stmt
        .query_map(params_refs.as_slice(), |row| {
            Ok(TransactionRow {
                posted: row.get("posted")?,
                amount: row.get("amount")?,
                description: row.get("description")?,
                pending: row.get::<_, i64>("pending")? != 0,
                category_name: row.get("category_name")?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    if transactions.is_empty() {
        println!("No transactions found.");
        return Ok(());
    }

    println!(
        "{:<12} {:>10}  {:<16} DESCRIPTION",
        "DATE", "AMOUNT", "CATEGORY"
    );
    println!("{}", "-".repeat(85));

    for tx in transactions {
        let date_str = Utc
            .timestamp_opt(tx.posted, 0)
            .single()
            .map(|dt| dt.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "?".to_string());

        let pending_marker = if tx.pending { "*" } else { "" };
        let category = tx.category_name.as_deref().unwrap_or("-");
        let amount = tx.amount as f64 / 100.0;

        println!(
            "{:<12} {:>10.2}  {:<16} {}{}",
            date_str,
            amount,
            truncate(category, 16),
            truncate(&tx.description, 38),
            pending_marker,
        );
    }

    Ok(())
}
