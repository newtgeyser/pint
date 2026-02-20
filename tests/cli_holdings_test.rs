//! Integration tests for holdings functionality.

mod common;

use pint::db::models::Holding;

#[test]
fn test_list_all_holdings() {
    let conn = common::setup_test_db();

    let holdings = Holding::find_all(&conn).unwrap();

    // Should have 7 holdings (4 brokerage + 3 retirement)
    assert_eq!(holdings.len(), 7);
}

#[test]
fn test_holdings_sorted_by_market_value() {
    let conn = common::setup_test_db();

    let holdings = Holding::find_all(&conn).unwrap();

    // Should be sorted by market value descending
    for i in 0..holdings.len() - 1 {
        let val_a = holdings[i].market_value.unwrap_or(0);
        let val_b = holdings[i + 1].market_value.unwrap_or(0);
        assert!(val_a >= val_b, "Holdings should be sorted by market value descending");
    }
}

#[test]
fn test_filter_holdings_by_account() {
    let conn = common::setup_test_db();

    // Filter by brokerage account
    let holdings = Holding::find_by_account_filter(&conn, "ACT-004-BROKERAGE").unwrap();
    assert_eq!(holdings.len(), 4);

    // Filter by 401k account
    let holdings = Holding::find_by_account_filter(&conn, "ACT-005-401K").unwrap();
    assert_eq!(holdings.len(), 3);
}

#[test]
fn test_filter_holdings_by_account_nickname() {
    let conn = common::setup_test_db();

    // Filter by nickname "Taxable"
    let holdings = Holding::find_by_account_filter(&conn, "Taxable").unwrap();
    assert_eq!(holdings.len(), 4);

    // Filter by nickname "401k"
    let holdings = Holding::find_by_account_filter(&conn, "401k").unwrap();
    assert_eq!(holdings.len(), 3);
}

#[test]
fn test_find_holding_by_id() {
    let conn = common::setup_test_db();

    let holding = Holding::find_by_query(&conn, "HOLD-001").unwrap();
    assert!(holding.is_some());

    let holding = holding.unwrap();
    assert_eq!(holding.symbol, Some("VTI".to_string()));
}

#[test]
fn test_find_holding_by_symbol() {
    let conn = common::setup_test_db();

    // Note: The current find_by_query doesn't search by symbol, but by ID
    // This test verifies the ID search works
    let holding = Holding::find_by_query(&conn, "HOLD-002").unwrap();
    assert!(holding.is_some());
    assert_eq!(holding.unwrap().symbol, Some("VXUS".to_string()));
}

#[test]
fn test_holding_price_in_cents() {
    let conn = common::setup_test_db();

    let holdings = Holding::find_all(&conn).unwrap();

    // Find VTI (price should be $225.00 = 22500 cents)
    let vti = holdings.iter().find(|h| h.symbol.as_deref() == Some("VTI")).unwrap();

    assert_eq!(vti.price, Some(22500));
    assert_eq!(vti.price_dollars(), Some(225.0));
}

#[test]
fn test_holding_market_value() {
    let conn = common::setup_test_db();

    let holdings = Holding::find_all(&conn).unwrap();

    // Find VTI (50 shares @ $225 = $11,250.00 market value)
    let vti = holdings.iter().find(|h| h.symbol.as_deref() == Some("VTI")).unwrap();

    assert_eq!(vti.market_value, Some(1125000)); // in cents
    assert_eq!(vti.market_value_dollars(), Some(11250.0));
}

#[test]
fn test_holding_cost_basis() {
    let conn = common::setup_test_db();

    let holdings = Holding::find_all(&conn).unwrap();

    // Find VTI (cost basis $10,000)
    let vti = holdings.iter().find(|h| h.symbol.as_deref() == Some("VTI")).unwrap();

    assert_eq!(vti.cost_basis, Some(1000000)); // in cents
    assert_eq!(vti.cost_basis_dollars(), Some(10000.0));
}

#[test]
fn test_holding_gain_calculation() {
    let conn = common::setup_test_db();

    let holdings = Holding::find_all(&conn).unwrap();

    // VTI: market value $11,250, cost basis $10,000 = $1,250 gain
    let vti = holdings.iter().find(|h| h.symbol.as_deref() == Some("VTI")).unwrap();

    let gain = vti.market_value_dollars().unwrap() - vti.cost_basis_dollars().unwrap();
    assert!((gain - 1250.0).abs() < 0.01);
}

#[test]
fn test_holding_shares() {
    let conn = common::setup_test_db();

    let holdings = Holding::find_all(&conn).unwrap();

    // VTI has 50 shares
    let vti = holdings.iter().find(|h| h.symbol.as_deref() == Some("VTI")).unwrap();
    assert_eq!(vti.shares, "50.000");

    // BND has 100 shares
    let bnd = holdings.iter().find(|h| h.symbol.as_deref() == Some("BND")).unwrap();
    assert_eq!(bnd.shares, "100.000");
}

#[test]
fn test_holding_description() {
    let conn = common::setup_test_db();

    let holdings = Holding::find_all(&conn).unwrap();

    let vti = holdings.iter().find(|h| h.symbol.as_deref() == Some("VTI")).unwrap();
    assert_eq!(vti.description, Some("Vanguard Total Stock Market ETF".to_string()));
}

#[test]
fn test_add_holding() {
    let conn = common::setup_test_db();

    let initial_count = Holding::find_all(&conn).unwrap().len();

    // Add a new holding
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT INTO holdings (id, account_id, symbol, description, shares, price, cost_basis, market_value, currency, created_at, updated_at)
         VALUES ('HOLD-NEW', 'ACT-004-BROKERAGE', 'GOOGL', 'Alphabet Inc.', '10.000', 15000, 140000, 150000, 'USD', ?1, ?1)",
        [now],
    ).unwrap();

    let holdings = Holding::find_all(&conn).unwrap();
    assert_eq!(holdings.len(), initial_count + 1);

    // Verify the new holding
    let googl = Holding::find_by_query(&conn, "HOLD-NEW").unwrap().unwrap();
    assert_eq!(googl.symbol, Some("GOOGL".to_string()));
    assert_eq!(googl.shares, "10.000");
}

#[test]
fn test_update_holding_price() {
    let conn = common::setup_test_db();

    // Update VTI price
    conn.execute(
        "UPDATE holdings SET price = 23000, market_value = 1150000 WHERE id = 'HOLD-001'",
        [],
    ).unwrap();

    let holding = Holding::find_by_query(&conn, "HOLD-001").unwrap().unwrap();
    assert_eq!(holding.price_dollars(), Some(230.0));
    assert_eq!(holding.market_value_dollars(), Some(11500.0));
}

#[test]
fn test_remove_holding() {
    let conn = common::setup_test_db();

    let initial_count = Holding::find_all(&conn).unwrap().len();

    conn.execute("DELETE FROM holdings WHERE id = 'HOLD-001'", []).unwrap();

    let holdings = Holding::find_all(&conn).unwrap();
    assert_eq!(holdings.len(), initial_count - 1);

    // Verify it's gone
    let holding = Holding::find_by_query(&conn, "HOLD-001").unwrap();
    assert!(holding.is_none());
}

#[test]
fn test_total_holdings_value() {
    let conn = common::setup_test_db();

    let holdings = Holding::find_all(&conn).unwrap();

    let total: i64 = holdings.iter()
        .filter_map(|h| h.market_value)
        .sum();

    // Brokerage: 1,125,000 + 435,000 + 720,000 + 437,500 = 2,717,500
    // 401k: 2,700,000 + 210,000 + 450,000 = 3,360,000
    // Total: 6,077,500 cents = $60,775.00
    assert_eq!(total, 6077500);
}

#[test]
fn test_holdings_currency() {
    let conn = common::setup_test_db();

    let holdings = Holding::find_all(&conn).unwrap();

    // All holdings should be in USD
    for holding in &holdings {
        assert_eq!(holding.currency, "USD");
    }
}
