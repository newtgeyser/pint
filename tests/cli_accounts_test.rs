//! Integration tests for accounts functionality.

mod common;

use chrono::Utc;
use pint::db::models::Account;

#[test]
fn test_list_accounts() {
    let conn = common::setup_test_db();

    let accounts = Account::find_all(&conn).unwrap();

    // Should have 6 accounts from synthetic data
    assert_eq!(accounts.len(), 6);

    // Check account types
    let checking_count = accounts
        .iter()
        .filter(|a| a.account_type == "checking")
        .count();
    let savings_count = accounts
        .iter()
        .filter(|a| a.account_type == "savings")
        .count();
    let credit_count = accounts
        .iter()
        .filter(|a| a.account_type == "credit")
        .count();
    let brokerage_count = accounts
        .iter()
        .filter(|a| a.account_type == "brokerage")
        .count();
    let retirement_count = accounts
        .iter()
        .filter(|a| a.account_type == "retirement")
        .count();

    assert_eq!(checking_count, 2); // Main checking + manual cash
    assert_eq!(savings_count, 1);
    assert_eq!(credit_count, 1);
    assert_eq!(brokerage_count, 1);
    assert_eq!(retirement_count, 1);
}

#[test]
fn test_find_account_by_id() {
    let conn = common::setup_test_db();

    // Find by full ID
    let account = Account::find_by_query(&conn, "ACT-001-CHECKING").unwrap();
    assert!(account.is_some());
    assert_eq!(account.unwrap().name, "Primary Checking Account");

    // Find by ID prefix
    let account = Account::find_by_query(&conn, "ACT-001").unwrap();
    assert!(account.is_some());
}

#[test]
fn test_find_account_by_nickname() {
    let conn = common::setup_test_db();

    // Find by nickname
    let account = Account::find_by_query(&conn, "Main Checking").unwrap();
    assert!(account.is_some());
    assert_eq!(account.unwrap().id, "ACT-001-CHECKING");

    // Find by partial nickname
    let account = Account::find_by_query(&conn, "Emergency").unwrap();
    assert!(account.is_some());
    assert_eq!(account.unwrap().account_type, "savings");
}

#[test]
fn test_find_account_by_name() {
    let conn = common::setup_test_db();

    // Find by name
    let account = Account::find_by_query(&conn, "Rewards Credit").unwrap();
    assert!(account.is_some());
    assert_eq!(account.unwrap().account_type, "credit");
}

#[test]
fn test_account_not_found() {
    let conn = common::setup_test_db();

    let account = Account::find_by_query(&conn, "nonexistent-account").unwrap();
    assert!(account.is_none());
}

#[test]
fn test_account_balance() {
    let conn = common::setup_test_db();

    let account = Account::find_by_query(&conn, "ACT-001-CHECKING")
        .unwrap()
        .unwrap();

    // Balance is $12,500.00 (1250000 cents)
    assert_eq!(account.balance, Some(1250000));
    assert_eq!(account.balance_dollars(), Some(12500.0));
}

#[test]
fn test_credit_card_negative_balance() {
    let conn = common::setup_test_db();

    let account = Account::find_by_query(&conn, "ACT-003-CREDIT")
        .unwrap()
        .unwrap();

    // Credit card balance is -$1,850.00
    assert_eq!(account.balance, Some(-185000));
    assert!(account.balance_dollars().unwrap() < 0.0);
}

#[test]
fn test_manual_account_flag() {
    let conn = common::setup_test_db();

    // Manual account
    let account = Account::find_by_query(&conn, "ACT-006-MANUAL")
        .unwrap()
        .unwrap();
    assert!(account.manual);

    // Non-manual account
    let account = Account::find_by_query(&conn, "ACT-001-CHECKING")
        .unwrap()
        .unwrap();
    assert!(!account.manual);
}

#[test]
fn test_add_manual_account() {
    let conn = common::setup_test_db();

    let initial_count = common::count_records(&conn, "accounts");

    // Add a new manual account
    let now = Utc::now().timestamp();
    conn.execute(
        "INSERT INTO accounts (id, name, account_type, currency, manual, created_at, updated_at)
         VALUES ('manual-test-123', 'Test Account', 'checking', 'USD', 1, ?1, ?1)",
        [now],
    )
    .unwrap();

    let new_count = common::count_records(&conn, "accounts");
    assert_eq!(new_count, initial_count + 1);

    // Verify the new account
    let account = Account::find_by_query(&conn, "manual-test-123")
        .unwrap()
        .unwrap();
    assert_eq!(account.name, "Test Account");
    assert!(account.manual);
}

#[test]
fn test_remove_account_cascades_transactions() {
    let conn = common::setup_test_db();

    // Count transactions for the checking account
    let tx_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM transactions WHERE account_id = 'ACT-001-CHECKING'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(tx_count > 0);

    // Delete the account and its transactions
    conn.execute(
        "DELETE FROM transactions WHERE account_id = 'ACT-001-CHECKING'",
        [],
    )
    .unwrap();
    conn.execute("DELETE FROM accounts WHERE id = 'ACT-001-CHECKING'", [])
        .unwrap();

    // Verify account is gone
    let account = Account::find_by_query(&conn, "ACT-001-CHECKING").unwrap();
    assert!(account.is_none());

    // Verify transactions are gone
    let tx_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM transactions WHERE account_id = 'ACT-001-CHECKING'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(tx_count, 0);
}

#[test]
fn test_set_account_type() {
    let conn = common::setup_test_db();

    // Change account type
    conn.execute(
        "UPDATE accounts SET account_type = 'savings' WHERE id = 'ACT-001-CHECKING'",
        [],
    )
    .unwrap();

    let account = Account::find_by_query(&conn, "ACT-001-CHECKING")
        .unwrap()
        .unwrap();
    assert_eq!(account.account_type, "savings");
}

#[test]
fn test_set_account_nickname() {
    let conn = common::setup_test_db();

    // Set nickname
    conn.execute(
        "UPDATE accounts SET nickname = 'My Primary' WHERE id = 'ACT-001-CHECKING'",
        [],
    )
    .unwrap();

    let account = Account::find_by_query(&conn, "ACT-001-CHECKING")
        .unwrap()
        .unwrap();
    assert_eq!(account.nickname, Some("My Primary".to_string()));
    assert_eq!(account.display_name(), "My Primary");

    // Clear nickname
    conn.execute(
        "UPDATE accounts SET nickname = NULL WHERE id = 'ACT-001-CHECKING'",
        [],
    )
    .unwrap();

    let account = Account::find_by_query(&conn, "ACT-001-CHECKING")
        .unwrap()
        .unwrap();
    assert!(account.nickname.is_none());
    assert_eq!(account.display_name(), "Primary Checking Account");
}

#[test]
fn test_accounts_for_select() {
    let conn = common::setup_test_db();

    let accounts = Account::find_all_for_select(&conn).unwrap();

    // Should have 6 accounts
    assert_eq!(accounts.len(), 6);

    // Each entry is (id, display_name)
    for (id, name) in &accounts {
        assert!(!id.is_empty());
        assert!(!name.is_empty());
    }
}

#[test]
fn test_total_balance_calculation() {
    let conn = common::setup_test_db();

    let accounts = Account::find_all(&conn).unwrap();

    let total: i64 = accounts.iter().filter_map(|a| a.balance).sum();

    // Expected total from synthetic data:
    // Checking: 1,250,000 + Savings: 2,500,000 + Credit: -185,000 +
    // Brokerage: 15,000,000 + 401k: 45,000,000 + Manual: 50,000 = 63,615,000 cents
    assert_eq!(total, 63615000);
}
