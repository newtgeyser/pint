//! Integration tests for summary and net worth calculations.

mod common;

use pint::db::models::{Account, Asset};

/// Helper to calculate summary data the same way the TUI does.
fn calculate_summary(conn: &rusqlite::Connection) -> SummaryData {
    let accounts = Account::find_all(conn).unwrap();
    let assets = Asset::find_all(conn).unwrap();

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
            _ => {}
        }
    }

    let assets_total: i64 = assets.iter().filter_map(|a| a.value).sum();
    let net_worth = cash + brokerage + retirement + assets_total + credit;

    SummaryData {
        cash,
        brokerage,
        retirement,
        assets: assets_total,
        credit,
        net_worth,
    }
}

#[derive(Debug)]
struct SummaryData {
    cash: i64,
    brokerage: i64,
    retirement: i64,
    assets: i64,
    credit: i64,
    net_worth: i64,
}

#[test]
fn test_cash_aggregation() {
    let conn = common::setup_test_db();
    let summary = calculate_summary(&conn);

    // Cash = Checking (1,250,000) + Savings (2,500,000) + Manual checking (50,000)
    // = 3,800,000 cents = $38,000
    assert_eq!(summary.cash, 3800000);
}

#[test]
fn test_brokerage_aggregation() {
    let conn = common::setup_test_db();
    let summary = calculate_summary(&conn);

    // Brokerage = 15,000,000 cents = $150,000
    assert_eq!(summary.brokerage, 15000000);
}

#[test]
fn test_retirement_aggregation() {
    let conn = common::setup_test_db();
    let summary = calculate_summary(&conn);

    // Retirement (401k) = 45,000,000 cents = $450,000
    assert_eq!(summary.retirement, 45000000);
}

#[test]
fn test_credit_aggregation() {
    let conn = common::setup_test_db();
    let summary = calculate_summary(&conn);

    // Credit = -185,000 cents = -$1,850 (negative balance)
    assert_eq!(summary.credit, -185000);
}

#[test]
fn test_assets_aggregation() {
    let conn = common::setup_test_db();
    let summary = calculate_summary(&conn);

    // Assets = House (55,000,000) + Car (2,200,000) + Art (1,500,000)
    // = 58,700,000 cents = $587,000
    assert_eq!(summary.assets, 58700000);
}

#[test]
fn test_net_worth_calculation() {
    let conn = common::setup_test_db();
    let summary = calculate_summary(&conn);

    // Net worth = cash + brokerage + retirement + assets + credit
    // = 3,800,000 + 15,000,000 + 45,000,000 + 58,700,000 + (-185,000)
    // = 122,315,000 cents = $1,223,150
    let expected = summary.cash + summary.brokerage + summary.retirement + summary.assets + summary.credit;
    assert_eq!(summary.net_worth, expected);
    assert_eq!(summary.net_worth, 122315000);
}

#[test]
fn test_net_worth_in_dollars() {
    let conn = common::setup_test_db();
    let summary = calculate_summary(&conn);

    let net_worth_dollars = summary.net_worth as f64 / 100.0;
    assert!((net_worth_dollars - 1223150.0).abs() < 0.01);
}

#[test]
fn test_summary_with_updated_account_balance() {
    let conn = common::setup_test_db();

    // Update checking account balance
    conn.execute(
        "UPDATE accounts SET balance = 2000000 WHERE id = 'ACT-001-CHECKING'",
        [],
    ).unwrap();

    let summary = calculate_summary(&conn);

    // Cash should now be: 2,000,000 + 2,500,000 + 50,000 = 4,550,000
    assert_eq!(summary.cash, 4550000);
}

#[test]
fn test_summary_with_added_account() {
    let conn = common::setup_test_db();

    // Add another savings account
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT INTO accounts (id, name, account_type, balance, currency, manual, created_at, updated_at)
         VALUES ('ACT-NEW-SAVINGS', 'New Savings', 'savings', 1000000, 'USD', 0, ?1, ?1)",
        [now],
    ).unwrap();

    let summary = calculate_summary(&conn);

    // Cash should now include the new savings: 3,800,000 + 1,000,000 = 4,800,000
    assert_eq!(summary.cash, 4800000);
}

#[test]
fn test_summary_with_added_asset() {
    let conn = common::setup_test_db();

    // Add a new asset
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT INTO assets (name, asset_type, value, currency, created_at, updated_at)
         VALUES ('Gold Coins', 'collectible', 500000, 'USD', ?1, ?1)",
        [now],
    ).unwrap();

    let summary = calculate_summary(&conn);

    // Assets should now be: 58,700,000 + 500,000 = 59,200,000
    assert_eq!(summary.assets, 59200000);
}

#[test]
fn test_summary_with_removed_account() {
    let conn = common::setup_test_db();

    // Remove savings account
    conn.execute("DELETE FROM accounts WHERE id = 'ACT-002-SAVINGS'", []).unwrap();

    let summary = calculate_summary(&conn);

    // Cash should now be: 1,250,000 + 50,000 = 1,300,000 (no savings)
    assert_eq!(summary.cash, 1300000);
}

#[test]
fn test_summary_with_null_balance() {
    let conn = common::setup_test_db();

    // Set an account balance to NULL
    conn.execute(
        "UPDATE accounts SET balance = NULL WHERE id = 'ACT-001-CHECKING'",
        [],
    ).unwrap();

    let summary = calculate_summary(&conn);

    // Cash should now be: 0 + 2,500,000 + 50,000 = 2,550,000 (NULL treated as 0)
    assert_eq!(summary.cash, 2550000);
}

#[test]
fn test_summary_with_null_asset_value() {
    let conn = common::setup_test_db();

    // Set an asset value to NULL
    conn.execute(
        "UPDATE assets SET value = NULL WHERE name = 'Art Collection'",
        [],
    ).unwrap();

    let summary = calculate_summary(&conn);

    // Assets should now be: 55,000,000 + 2,200,000 + 0 = 57,200,000
    assert_eq!(summary.assets, 57200000);
}

#[test]
fn test_summary_credit_reduces_net_worth() {
    let conn = common::setup_test_db();

    // Increase credit card debt
    conn.execute(
        "UPDATE accounts SET balance = -500000 WHERE id = 'ACT-003-CREDIT'",
        [],
    ).unwrap();

    let summary = calculate_summary(&conn);

    // Credit should be -500,000 and reduce net worth accordingly
    assert_eq!(summary.credit, -500000);

    // Net worth = 3,800,000 + 15,000,000 + 45,000,000 + 58,700,000 + (-500,000)
    // = 122,000,000
    assert_eq!(summary.net_worth, 122000000);
}

#[test]
fn test_summary_multiple_account_types() {
    let conn = common::setup_test_db();

    // Add a second brokerage account
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT INTO accounts (id, name, account_type, balance, currency, manual, created_at, updated_at)
         VALUES ('ACT-NEW-BROKERAGE', 'Roth IRA', 'brokerage', 5000000, 'USD', 0, ?1, ?1)",
        [now],
    ).unwrap();

    let summary = calculate_summary(&conn);

    // Brokerage should now be: 15,000,000 + 5,000,000 = 20,000,000
    assert_eq!(summary.brokerage, 20000000);
}

#[test]
fn test_summary_unknown_account_type_excluded() {
    let conn = common::setup_test_db();

    // Add an account with unknown type
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT INTO accounts (id, name, account_type, balance, currency, manual, created_at, updated_at)
         VALUES ('ACT-UNKNOWN', 'Mystery Account', 'loan', 1000000, 'USD', 0, ?1, ?1)",
        [now],
    ).unwrap();

    let summary = calculate_summary(&conn);

    // Net worth should be unchanged - loan type is excluded
    assert_eq!(summary.net_worth, 122315000);
}

#[test]
fn test_empty_database_summary() {
    let conn = pint::db::open_in_memory().unwrap();

    let summary = calculate_summary(&conn);

    assert_eq!(summary.cash, 0);
    assert_eq!(summary.brokerage, 0);
    assert_eq!(summary.retirement, 0);
    assert_eq!(summary.assets, 0);
    assert_eq!(summary.credit, 0);
    assert_eq!(summary.net_worth, 0);
}
