use std::collections::HashMap;

use anyhow::{Result, bail};
use chrono::Utc;
use rusqlite::{Connection, params};

use super::MatchMode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MerchantGroup {
    pub merchant: String,
    pub count: usize,
    pub net_amount: i64,
    pub newest_posted: i64,
    pub example: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategorizeGroupResult {
    pub categorized: usize,
    pub rule_pattern: Option<String>,
}

/// Produce a stable merchant key while discarding common payment-channel and
/// transaction-reference noise. The original description remains available as
/// the group's example and can be used as an explicit rule pattern.
pub fn normalize_merchant(description: &str) -> String {
    const NOISE: &[&str] = &[
        "ACH",
        "CARD",
        "CHECKCARD",
        "DEBIT",
        "ONLINE",
        "PAYMENT",
        "POS",
        "PURCHASE",
        "RECURRING",
        "TRANSACTION",
    ];

    let cleaned = description
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(str::to_ascii_uppercase)
        .filter(|token| !NOISE.contains(&token.as_str()))
        .filter(|token| !token.chars().all(|c| c.is_ascii_digit()))
        .filter(|token| {
            token.len() < 6
                || !(token.chars().any(|c| c.is_ascii_digit())
                    && token.chars().any(|c| c.is_ascii_alphabetic()))
        })
        .collect::<Vec<_>>()
        .join(" ");

    if cleaned.is_empty() {
        description.trim().to_ascii_uppercase()
    } else {
        cleaned
    }
}

pub fn list_groups(conn: &Connection, limit: Option<usize>) -> Result<Vec<MerchantGroup>> {
    let mut stmt = conn.prepare(
        "SELECT description, amount, posted
         FROM transactions
         WHERE category_id IS NULL
         ORDER BY posted DESC, id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;

    let mut groups: HashMap<String, MerchantGroup> = HashMap::new();
    for row in rows {
        let (description, amount, posted) = row?;
        let merchant = normalize_merchant(&description);
        let group = groups
            .entry(merchant.clone())
            .or_insert_with(|| MerchantGroup {
                merchant,
                count: 0,
                net_amount: 0,
                newest_posted: posted,
                example: description.clone(),
            });
        group.count += 1;
        group.net_amount += amount;
        if posted > group.newest_posted {
            group.newest_posted = posted;
            group.example = description;
        }
    }

    let mut groups = groups.into_values().collect::<Vec<_>>();
    groups.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| b.newest_posted.cmp(&a.newest_posted))
            .then_with(|| a.merchant.cmp(&b.merchant))
    });
    if let Some(limit) = limit {
        groups.truncate(limit);
    }
    Ok(groups)
}

/// Categorize every currently uncategorized transaction matching `merchant`.
/// Matching is performed using the same normalization as `list_groups`, inside
/// one transaction. An optional rule is created only when at least one row was
/// categorized.
pub fn categorize_group(
    conn: &mut Connection,
    merchant: &str,
    category_name: &str,
    rule_pattern: Option<&str>,
) -> Result<CategorizeGroupResult> {
    let merchant = normalize_merchant(merchant);
    let category_name = category_name.trim();
    if merchant.is_empty() {
        bail!("merchant cannot be empty");
    }
    if category_name.is_empty() {
        bail!("category cannot be empty");
    }

    let tx = conn.transaction()?;
    let mut matches = Vec::new();
    {
        let mut stmt =
            tx.prepare("SELECT id, description FROM transactions WHERE category_id IS NULL")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (id, description) = row?;
            if normalize_merchant(&description) == merchant {
                matches.push(id);
            }
        }
    }
    if matches.is_empty() {
        bail!("uncategorized merchant group '{}' not found", merchant);
    }

    let now = Utc::now().timestamp();
    let category_id = tx
        .query_row(
            "SELECT id FROM categories WHERE name = ?1 COLLATE NOCASE",
            [category_name],
            |row| row.get::<_, i64>(0),
        )
        .or_else(|error| {
            if matches!(error, rusqlite::Error::QueryReturnedNoRows) {
                tx.execute(
                    "INSERT INTO categories (name, created_at) VALUES (?1, ?2)",
                    params![category_name, now],
                )?;
                Ok(tx.last_insert_rowid())
            } else {
                Err(error)
            }
        })?;

    for id in &matches {
        tx.execute(
            "UPDATE transactions
             SET category_id = ?1, updated_at = ?2
             WHERE id = ?3 AND category_id IS NULL",
            params![category_id, now, id],
        )?;
    }

    let rule_pattern = rule_pattern
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(pattern) = rule_pattern {
        let pattern = pattern.to_ascii_uppercase();
        tx.execute(
            "INSERT INTO merchant_rules (pattern, match_mode, category_id, created_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(pattern) DO UPDATE SET
                match_mode = excluded.match_mode,
                category_id = excluded.category_id",
            params![pattern, MatchMode::Substring.as_str(), category_id, now],
        )?;
    }
    tx.commit()?;

    Ok(CategorizeGroupResult {
        categorized: matches.len(),
        rule_pattern: rule_pattern.map(str::to_ascii_uppercase),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn insert_account(conn: &Connection) {
        conn.execute(
            "INSERT INTO accounts
             (id, name, account_type, currency, manual, created_at, updated_at)
             VALUES ('account', 'Checking', 'checking', 'USD', 0, 1, 1)",
            [],
        )
        .unwrap();
    }

    fn insert_transaction(
        conn: &Connection,
        id: &str,
        description: &str,
        amount: i64,
        posted: i64,
    ) {
        conn.execute(
            "INSERT INTO transactions
             (id, account_id, posted, amount, description, pending, created_at, updated_at)
             VALUES (?1, 'account', ?2, ?3, ?4, 0, 1, 1)",
            params![id, posted, amount, description],
        )
        .unwrap();
    }

    #[test]
    fn normalizes_channel_noise_and_references() {
        assert_eq!(
            normalize_merchant("POS PURCHASE Acme Coffee #1234 A91B2345"),
            "ACME COFFEE"
        );
        assert_eq!(
            normalize_merchant("Online payment 2026-01-10"),
            "ONLINE PAYMENT 2026-01-10"
        );
    }

    #[test]
    fn groups_and_sorts_uncategorized_transactions() {
        let conn = crate::db::open_in_memory().unwrap();
        insert_account(&conn);
        insert_transaction(&conn, "1", "POS ACME COFFEE 1234", -500, 10);
        insert_transaction(&conn, "2", "Debit Acme Coffee #9999", -700, 20);
        insert_transaction(&conn, "3", "OTHER SHOP", -300, 30);

        let groups = list_groups(&conn, None).unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].merchant, "ACME COFFEE");
        assert_eq!(groups[0].count, 2);
        assert_eq!(groups[0].net_amount, -1200);
        assert_eq!(groups[0].newest_posted, 20);
        assert_eq!(groups[0].example, "Debit Acme Coffee #9999");
    }

    #[test]
    fn categorizes_group_and_optionally_creates_rule_atomically() {
        let mut conn = crate::db::open_in_memory().unwrap();
        insert_account(&conn);
        insert_transaction(&conn, "1", "POS ACME COFFEE 1234", -500, 10);
        insert_transaction(&conn, "2", "ACME COFFEE #9999", -700, 20);
        insert_transaction(&conn, "3", "OTHER SHOP", -300, 30);

        let result =
            categorize_group(&mut conn, "acme coffee", "Dining", Some("ACME COFFEE")).unwrap();
        assert_eq!(result.categorized, 2);
        assert_eq!(result.rule_pattern.as_deref(), Some("ACME COFFEE"));

        let categorized: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM transactions t
                 JOIN categories c ON c.id = t.category_id
                 WHERE c.name = 'Dining'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(categorized, 2);
        let rule_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM merchant_rules WHERE pattern = 'ACME COFFEE'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(rule_count, 1);
    }
}
