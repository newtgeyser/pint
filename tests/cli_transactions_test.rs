//! Integration tests for transactions functionality.

mod common;

use pint::db::models::{Transaction, TransactionRow};

#[test]
fn test_list_all_transactions() {
    let conn = common::setup_test_db();

    let transactions = TransactionRow::find_all(&conn, None, 1000).unwrap();

    // Should have many transactions from synthetic data
    assert!(transactions.len() > 50);
}

#[test]
fn test_list_transactions_with_limit() {
    let conn = common::setup_test_db();

    let transactions = TransactionRow::find_all(&conn, None, 10).unwrap();
    assert_eq!(transactions.len(), 10);
}

#[test]
fn test_filter_transactions_by_account() {
    let conn = common::setup_test_db();

    // Filter by checking account
    let transactions = TransactionRow::find_all(&conn, Some("ACT-001-CHECKING"), 1000).unwrap();

    // Should have mortgage, electric, internet, paycheck, transfer transactions
    assert!(transactions.len() > 10);

    // All transactions should be from the checking account
    for tx in &transactions {
        assert!(tx.account_name.contains("Checking") || tx.account_name.contains("Main"));
    }
}

#[test]
fn test_filter_transactions_by_account_nickname() {
    let conn = common::setup_test_db();

    // Filter by nickname
    let transactions = TransactionRow::find_all(&conn, Some("Chase"), 1000).unwrap();

    // Should have credit card transactions
    assert!(transactions.len() > 0);
}

#[test]
fn test_transactions_ordered_by_date() {
    let conn = common::setup_test_db();

    let transactions = TransactionRow::find_all(&conn, None, 100).unwrap();

    // Should be ordered by posted date descending (most recent first)
    for i in 0..transactions.len() - 1 {
        assert!(
            transactions[i].date >= transactions[i + 1].date,
            "Transactions should be ordered by date descending"
        );
    }
}

#[test]
fn test_transaction_amount_formatting() {
    let conn = common::setup_test_db();

    let transactions = TransactionRow::find_all(&conn, None, 100).unwrap();

    // Find a mortgage transaction (should be -$2,450.00)
    let mortgage = transactions
        .iter()
        .find(|t| t.description.contains("MORTGAGE"))
        .expect("Should find mortgage transaction");

    assert_eq!(mortgage.amount, -2450.0);
}

#[test]
fn test_categorized_transactions() {
    let conn = common::setup_test_db();

    let transactions = TransactionRow::find_all(&conn, None, 1000).unwrap();

    // Count categorized vs uncategorized
    let categorized = transactions.iter().filter(|t| t.category.is_some()).count();
    let uncategorized = transactions.iter().filter(|t| t.category.is_none()).count();

    // Most transactions should be categorized
    assert!(categorized > uncategorized);

    // Check specific categories exist
    let has_mortgage = transactions
        .iter()
        .any(|t| t.category.as_deref() == Some("Mortgage"));
    let has_utilities = transactions
        .iter()
        .any(|t| t.category.as_deref() == Some("Utilities"));
    let has_groceries = transactions
        .iter()
        .any(|t| t.category.as_deref() == Some("Groceries"));

    assert!(has_mortgage);
    assert!(has_utilities);
    assert!(has_groceries);
}

#[test]
fn test_uncategorized_transactions() {
    let conn = common::setup_test_db();

    let transactions = TransactionRow::find_all(&conn, None, 1000).unwrap();

    // Should have some uncategorized transactions (Amazon, ATM, Venmo, etc.)
    let uncategorized: Vec<_> = transactions
        .iter()
        .filter(|t| t.category.is_none())
        .collect();

    assert!(uncategorized.len() > 0);

    // Check specific uncategorized merchants
    let has_amazon = uncategorized
        .iter()
        .any(|t| t.description.contains("AMAZON"));
    assert!(has_amazon);
}

#[test]
fn test_pending_transactions() {
    let conn = common::setup_test_db();

    let transactions = TransactionRow::find_all(&conn, None, 1000).unwrap();

    // Should have one pending transaction
    let pending: Vec<_> = transactions.iter().filter(|t| t.pending).collect();

    assert_eq!(pending.len(), 1);
    assert!(pending[0].description.contains("PENDING"));
}

#[test]
fn test_set_transaction_category() {
    let conn = common::setup_test_db();

    // Find an uncategorized transaction
    let tx_id = "TX-UNCAT-0";

    // Get the Shopping category ID
    let category_id = common::get_category_id(&conn, "Shopping").unwrap();

    // Set the category
    Transaction::set_category(&conn, tx_id, category_id).unwrap();

    // Verify the category was set
    let transactions = TransactionRow::find_all(&conn, None, 1000).unwrap();
    let updated_tx = transactions
        .iter()
        .find(|t| t.id == tx_id)
        .expect("Should find the transaction");

    assert_eq!(updated_tx.category.as_deref(), Some("Shopping"));
}

#[test]
fn test_transaction_search_by_description() {
    let conn = common::setup_test_db();

    // Search for Netflix transactions using SQL LIKE
    let transactions: Vec<TransactionRow> = {
        let mut stmt = conn
            .prepare(
                "SELECT t.id, t.posted, t.amount, t.description, t.pending, c.name as category,
                    COALESCE(a.nickname, a.name) as account_name
             FROM transactions t
             LEFT JOIN categories c ON t.category_id = c.id
             JOIN accounts a ON t.account_id = a.id
             WHERE LOWER(t.description) LIKE '%netflix%'
             ORDER BY t.posted DESC",
            )
            .unwrap();

        stmt.query_map([], |row| {
            let posted: i64 = row.get(1)?;
            let date = chrono::Utc
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
                reimburser: None,
                reimbursed_at: None,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
    };

    // Should find Netflix subscriptions (6 monthly)
    assert_eq!(transactions.len(), 6);
    assert!(
        transactions
            .iter()
            .all(|t| t.description.contains("NETFLIX"))
    );
}

#[test]
fn test_transaction_amounts_in_cents() {
    let conn = common::setup_test_db();

    // Verify amounts are stored in cents
    let amount: i64 = conn
        .query_row(
            "SELECT amount FROM transactions WHERE description LIKE '%NETFLIX%' LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();

    // Netflix is $15.99 = -1599 cents
    assert_eq!(amount, -1599);
}

#[test]
fn test_transactions_with_account_name() {
    let conn = common::setup_test_db();

    let transactions = TransactionRow::find_all(&conn, None, 100).unwrap();

    // All transactions should have account names
    for tx in &transactions {
        assert!(!tx.account_name.is_empty());
    }

    // Check that nicknames are used when available
    let credit_txs: Vec<_> = transactions
        .iter()
        .filter(|t| t.account_name == "Chase Card")
        .collect();

    assert!(
        credit_txs.len() > 0,
        "Should find transactions with nickname 'Chase Card'"
    );
}

use chrono::TimeZone;
