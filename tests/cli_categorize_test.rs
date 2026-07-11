//! Integration tests for categorization workflows.

mod common;

use pint::commands::rules::auto_categorize_all;
use pint::db::models::TransactionRow;
use pint::db::{self, MatchMode};

// =============================================================================
// Auto-categorize tests
// =============================================================================

#[test]
fn test_auto_categorize_uncategorized_transactions() {
    let conn = common::setup_test_db();

    // Add an uncategorized transaction that matches an existing rule (STARBUCKS -> Dining)
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT INTO transactions (id, account_id, posted, amount, description, pending, category_id, created_at, updated_at)
         VALUES ('TX-UNCAT-STARBUCKS', 'ACT-003-CREDIT', ?1, -550, 'STARBUCKS STORE #99999', 0, NULL, ?1, ?1)",
        [now],
    ).unwrap();

    // Count uncategorized before
    let uncategorized_before: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM transactions WHERE category_id IS NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(
        uncategorized_before > 0,
        "Should have uncategorized transactions"
    );

    // Run auto-categorization
    let categorized = auto_categorize_all(&conn).unwrap();

    // Should have categorized at least the Starbucks transaction
    assert!(categorized > 0, "Should have categorized some transactions");

    // Count uncategorized after
    let uncategorized_after: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM transactions WHERE category_id IS NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();

    assert!(
        uncategorized_after < uncategorized_before,
        "Should have fewer uncategorized transactions after auto-categorize"
    );

    // Verify the Starbucks transaction was categorized as Dining
    let txs = TransactionRow::find_all(&conn, None, 1000).unwrap();
    let tx = txs.iter().find(|t| t.id == "TX-UNCAT-STARBUCKS").unwrap();
    assert_eq!(tx.category.as_deref(), Some("Dining"));
}

#[test]
fn test_auto_categorize_applies_substring_rules() {
    let conn = common::setup_test_db();

    // Add an uncategorized Amazon transaction
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT INTO transactions (id, account_id, posted, amount, description, pending, category_id, created_at, updated_at)
         VALUES ('TX-TEST-AMAZON', 'ACT-003-CREDIT', ?1, -5000, 'AMAZON.COM PURCHASE', 0, NULL, ?1, ?1)",
        [now],
    ).unwrap();

    // Add a rule for Amazon -> Shopping
    let shopping_id = common::get_category_id(&conn, "Shopping").unwrap();
    db::upsert_merchant_rule_with_mode(&conn, "AMAZON", shopping_id, MatchMode::Substring).unwrap();

    // Run auto-categorization
    let categorized = auto_categorize_all(&conn).unwrap();
    assert!(
        categorized >= 1,
        "Should categorize at least the Amazon transaction"
    );

    // Verify it was categorized
    let txs = TransactionRow::find_all(&conn, None, 1000).unwrap();
    let tx = txs.iter().find(|t| t.id == "TX-TEST-AMAZON").unwrap();
    assert_eq!(tx.category.as_deref(), Some("Shopping"));
}

#[test]
fn test_auto_categorize_applies_token_rules() {
    let conn = common::setup_test_db();

    // Add an uncategorized transaction with GEICO at word boundary
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT INTO transactions (id, account_id, posted, amount, description, pending, category_id, created_at, updated_at)
         VALUES ('TX-TEST-GEICO', 'ACT-001-CHECKING', ?1, -15000, 'GEICO PAYMENT', 0, NULL, ?1, ?1)",
        [now],
    ).unwrap();

    // GEICO rule already exists with token match from synthetic data
    let categorized = auto_categorize_all(&conn).unwrap();
    assert!(categorized >= 1);

    // Verify it was categorized as Insurance
    let txs = TransactionRow::find_all(&conn, None, 1000).unwrap();
    let tx = txs.iter().find(|t| t.id == "TX-TEST-GEICO").unwrap();
    assert_eq!(tx.category.as_deref(), Some("Insurance"));
}

#[test]
fn test_auto_categorize_skips_already_categorized() {
    let conn = common::setup_test_db();

    // Get count of categorized transactions before (just to verify test setup)
    let _categorized_before: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM transactions WHERE category_id IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();

    // Run auto-categorization
    auto_categorize_all(&conn).unwrap();

    // Already categorized transactions should not be affected
    // (their category shouldn't change)
    let mortgage_tx = TransactionRow::find_all(&conn, None, 1000)
        .unwrap()
        .into_iter()
        .find(|t| t.description.contains("WELLS FARGO"))
        .unwrap();
    assert_eq!(mortgage_tx.category.as_deref(), Some("Mortgage"));
}

#[test]
fn test_auto_categorize_no_matching_rules() {
    let conn = common::setup_test_db();

    // Add a transaction that doesn't match any rule
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT INTO transactions (id, account_id, posted, amount, description, pending, category_id, created_at, updated_at)
         VALUES ('TX-NO-MATCH', 'ACT-003-CREDIT', ?1, -5000, 'COMPLETELY UNIQUE MERCHANT XYZ123', 0, NULL, ?1, ?1)",
        [now],
    ).unwrap();

    auto_categorize_all(&conn).unwrap();

    // Should still be uncategorized
    let txs = TransactionRow::find_all(&conn, None, 1000).unwrap();
    let tx = txs.iter().find(|t| t.id == "TX-NO-MATCH").unwrap();
    assert!(tx.category.is_none());
}

#[test]
fn test_auto_categorize_with_no_rules() {
    let conn = pint::db::open_in_memory().unwrap();

    // Add an uncategorized transaction without any rules
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT INTO accounts (id, name, account_type, currency, manual, created_at, updated_at)
         VALUES ('ACT-TEST', 'Test Account', 'checking', 'USD', 0, ?1, ?1)",
        [now],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO transactions (id, account_id, posted, amount, description, pending, category_id, created_at, updated_at)
         VALUES ('TX-TEST', 'ACT-TEST', ?1, -1000, 'TEST MERCHANT', 0, NULL, ?1, ?1)",
        [now],
    ).unwrap();

    // Should not panic, just return 0 categorized
    let categorized = auto_categorize_all(&conn).unwrap();
    assert_eq!(categorized, 0);
}

// =============================================================================
// Manual categorization tests
// =============================================================================

#[test]
fn test_categorize_single_transaction() {
    let conn = common::setup_test_db();

    let tx_id = "TX-UNCAT-0";
    let shopping_id = common::get_category_id(&conn, "Shopping").unwrap();

    // Verify uncategorized initially
    let txs = TransactionRow::find_all(&conn, None, 1000).unwrap();
    let tx = txs.iter().find(|t| t.id == tx_id).unwrap();
    assert!(tx.category.is_none());

    // Categorize it
    let result = db::categorize_transaction(&conn, tx_id, shopping_id).unwrap();
    assert!(result, "Should return true for successful categorization");

    // Verify categorized
    let txs = TransactionRow::find_all(&conn, None, 1000).unwrap();
    let tx = txs.iter().find(|t| t.id == tx_id).unwrap();
    assert_eq!(tx.category.as_deref(), Some("Shopping"));
}

#[test]
fn test_categorize_nonexistent_transaction() {
    let conn = common::setup_test_db();

    let shopping_id = common::get_category_id(&conn, "Shopping").unwrap();

    // Try to categorize a nonexistent transaction
    let result = db::categorize_transaction(&conn, "TX-DOES-NOT-EXIST", shopping_id).unwrap();
    assert!(!result, "Should return false for nonexistent transaction");
}

#[test]
fn test_recategorize_transaction() {
    let conn = common::setup_test_db();

    // Find a transaction already categorized as Mortgage
    let txs = TransactionRow::find_all(&conn, None, 1000).unwrap();
    let mortgage_tx = txs
        .iter()
        .find(|t| t.category.as_deref() == Some("Mortgage"))
        .unwrap();
    let tx_id = mortgage_tx.id.clone();

    // Recategorize as Entertainment
    let entertainment_id = common::get_category_id(&conn, "Entertainment").unwrap();
    db::categorize_transaction(&conn, &tx_id, entertainment_id).unwrap();

    // Verify new category
    let txs = TransactionRow::find_all(&conn, None, 1000).unwrap();
    let tx = txs.iter().find(|t| t.id == tx_id).unwrap();
    assert_eq!(tx.category.as_deref(), Some("Entertainment"));
}

// =============================================================================
// Category management tests
// =============================================================================

#[test]
fn test_get_existing_category() {
    let conn = common::setup_test_db();

    // Get an existing category
    let id = db::get_or_create_category(&conn, "Shopping").unwrap();
    assert!(id > 0);

    // Getting it again should return the same ID
    let id2 = db::get_or_create_category(&conn, "Shopping").unwrap();
    assert_eq!(id, id2);
}

#[test]
fn test_create_new_category() {
    let conn = common::setup_test_db();

    let initial_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM categories", [], |row| row.get(0))
        .unwrap();

    // Create a new category
    let id = db::get_or_create_category(&conn, "Pets").unwrap();
    assert!(id > 0);

    // Verify it was created
    let new_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM categories", [], |row| row.get(0))
        .unwrap();
    assert_eq!(new_count, initial_count + 1);

    // Verify we can find it
    let name: String = conn
        .query_row("SELECT name FROM categories WHERE id = ?1", [id], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(name, "Pets");
}

#[test]
fn test_category_case_sensitive() {
    let conn = common::setup_test_db();

    // Create "pets"
    let id1 = db::get_or_create_category(&conn, "pets").unwrap();

    // Create "Pets" - should be different
    let id2 = db::get_or_create_category(&conn, "Pets").unwrap();

    assert_ne!(id1, id2, "Categories should be case-sensitive");
}

// =============================================================================
// Rule management tests
// =============================================================================

#[test]
fn test_add_substring_rule() {
    let conn = common::setup_test_db();

    let shopping_id = common::get_category_id(&conn, "Shopping").unwrap();

    // Add a new rule
    db::upsert_merchant_rule_with_mode(&conn, "walmart", shopping_id, MatchMode::Substring)
        .unwrap();

    // Verify it exists (pattern should be uppercased)
    let rules = db::list_merchant_rules(&conn).unwrap();
    let walmart_rule = rules.iter().find(|r| r.pattern == "WALMART");
    assert!(walmart_rule.is_some());
    assert_eq!(walmart_rule.unwrap().match_mode, MatchMode::Substring);
}

#[test]
fn test_add_token_rule() {
    let conn = common::setup_test_db();

    let insurance_id = common::get_category_id(&conn, "Insurance").unwrap();

    // Add a token rule
    db::upsert_merchant_rule_with_mode(&conn, "allstate", insurance_id, MatchMode::Token).unwrap();

    // Verify it exists
    let rules = db::list_merchant_rules(&conn).unwrap();
    let rule = rules.iter().find(|r| r.pattern == "ALLSTATE");
    assert!(rule.is_some());
    assert_eq!(rule.unwrap().match_mode, MatchMode::Token);
}

#[test]
fn test_update_rule_category() {
    let conn = common::setup_test_db();

    // NETFLIX rule exists, change its category
    let entertainment_id = common::get_category_id(&conn, "Entertainment").unwrap();
    db::upsert_merchant_rule_with_mode(&conn, "NETFLIX", entertainment_id, MatchMode::Substring)
        .unwrap();

    // Verify the update
    let rules = db::list_merchant_rules(&conn).unwrap();
    let netflix = rules.iter().find(|r| r.pattern == "NETFLIX").unwrap();
    assert_eq!(netflix.category_id, entertainment_id);
}

#[test]
fn test_update_rule_match_mode() {
    let conn = common::setup_test_db();

    // NETFLIX uses substring, change to token
    let subs_id = common::get_category_id(&conn, "Subscriptions").unwrap();
    db::upsert_merchant_rule_with_mode(&conn, "NETFLIX", subs_id, MatchMode::Token).unwrap();

    // Verify the update
    let rules = db::list_merchant_rules(&conn).unwrap();
    let netflix = rules.iter().find(|r| r.pattern == "NETFLIX").unwrap();
    assert_eq!(netflix.match_mode, MatchMode::Token);
}

#[test]
fn test_rule_pattern_normalized() {
    let conn = common::setup_test_db();

    let shopping_id = common::get_category_id(&conn, "Shopping").unwrap();

    // Add rule with lowercase and whitespace
    db::upsert_merchant_rule_with_mode(&conn, "  best buy  ", shopping_id, MatchMode::Substring)
        .unwrap();

    // Verify it's normalized (uppercase, trimmed)
    let rules = db::list_merchant_rules(&conn).unwrap();
    let rule = rules.iter().find(|r| r.pattern == "BEST BUY");
    assert!(rule.is_some());
}

// =============================================================================
// Rule matching tests
// =============================================================================

#[test]
fn test_find_category_for_description_substring() {
    let conn = common::setup_test_db();

    let rules = db::list_merchant_rules(&conn).unwrap();

    // Should match Netflix (substring)
    let category_id =
        db::find_category_for_description_with_rules("NETFLIX STREAMING SERVICE", &rules);
    assert!(category_id.is_some());

    let subs_id = common::get_category_id(&conn, "Subscriptions").unwrap();
    assert_eq!(category_id.unwrap(), subs_id);
}

#[test]
fn test_find_category_for_description_token() {
    let conn = common::setup_test_db();

    let rules = db::list_merchant_rules(&conn).unwrap();

    // Should match GEICO (token)
    let category_id = db::find_category_for_description_with_rules("GEICO AUTO PAYMENT", &rules);
    assert!(category_id.is_some());

    let insurance_id = common::get_category_id(&conn, "Insurance").unwrap();
    assert_eq!(category_id.unwrap(), insurance_id);
}

#[test]
fn test_find_category_token_no_match_inside_word() {
    let conn = common::setup_test_db();

    let rules = db::list_merchant_rules(&conn).unwrap();

    // Should NOT match GEICO inside another word
    let category_id = db::find_category_for_description_with_rules("MYGEICOPAYMENT", &rules);

    // Either no match, or not Insurance category
    let insurance_id = common::get_category_id(&conn, "Insurance").unwrap();
    assert!(category_id.is_none() || category_id.unwrap() != insurance_id);
}

#[test]
fn test_find_category_case_insensitive() {
    let conn = common::setup_test_db();

    let rules = db::list_merchant_rules(&conn).unwrap();

    // Should match regardless of case
    let cat1 = db::find_category_for_description_with_rules("netflix", &rules);
    let cat2 = db::find_category_for_description_with_rules("NETFLIX", &rules);
    let cat3 = db::find_category_for_description_with_rules("Netflix", &rules);

    assert!(cat1.is_some());
    assert_eq!(cat1, cat2);
    assert_eq!(cat2, cat3);
}

#[test]
fn test_find_category_no_match() {
    let conn = common::setup_test_db();

    let rules = db::list_merchant_rules(&conn).unwrap();

    let category_id =
        db::find_category_for_description_with_rules("COMPLETELY UNKNOWN MERCHANT", &rules);
    assert!(category_id.is_none());
}

// =============================================================================
// Learn workflow tests (categorize + create rule)
// =============================================================================

#[test]
fn test_learn_workflow() {
    let conn = common::setup_test_db();

    // Add an uncategorized transaction
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT INTO transactions (id, account_id, posted, amount, description, pending, category_id, created_at, updated_at)
         VALUES ('TX-LEARN-TEST', 'ACT-003-CREDIT', ?1, -2500, 'PETCO STORE #123', 0, NULL, ?1, ?1)",
        [now],
    ).unwrap();

    // Create category and categorize
    let pets_id = db::get_or_create_category(&conn, "Pets").unwrap();
    db::categorize_transaction(&conn, "TX-LEARN-TEST", pets_id).unwrap();

    // Create rule
    db::upsert_merchant_rule_with_mode(&conn, "PETCO", pets_id, MatchMode::Substring).unwrap();

    // Verify transaction is categorized
    let txs = TransactionRow::find_all(&conn, None, 1000).unwrap();
    let tx = txs.iter().find(|t| t.id == "TX-LEARN-TEST").unwrap();
    assert_eq!(tx.category.as_deref(), Some("Pets"));

    // Verify rule exists
    let rules = db::list_merchant_rules(&conn).unwrap();
    assert!(rules.iter().any(|r| r.pattern == "PETCO"));

    // Verify future transactions would match
    let category_id = db::find_category_for_description_with_rules("PETCO GROOMING", &rules);
    assert_eq!(category_id, Some(pets_id));
}

#[test]
fn test_learn_workflow_with_existing_category() {
    let conn = common::setup_test_db();

    // Add an uncategorized transaction
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT INTO transactions (id, account_id, posted, amount, description, pending, category_id, created_at, updated_at)
         VALUES ('TX-LEARN-TEST2', 'ACT-003-CREDIT', ?1, -8000, 'TARGET STORE #456', 0, NULL, ?1, ?1)",
        [now],
    ).unwrap();

    // Use existing Shopping category
    let shopping_id = common::get_category_id(&conn, "Shopping").unwrap();

    // Categorize and create rule
    db::categorize_transaction(&conn, "TX-LEARN-TEST2", shopping_id).unwrap();
    db::upsert_merchant_rule_with_mode(&conn, "TARGET", shopping_id, MatchMode::Substring).unwrap();

    // Verify
    let txs = TransactionRow::find_all(&conn, None, 1000).unwrap();
    let tx = txs.iter().find(|t| t.id == "TX-LEARN-TEST2").unwrap();
    assert_eq!(tx.category.as_deref(), Some("Shopping"));

    // Future transactions should auto-categorize
    conn.execute(
        "INSERT INTO transactions (id, account_id, posted, amount, description, pending, category_id, created_at, updated_at)
         VALUES ('TX-FUTURE-TARGET', 'ACT-003-CREDIT', ?1, -3000, 'TARGET STORE #789', 0, NULL, ?1, ?1)",
        [now],
    ).unwrap();

    auto_categorize_all(&conn).unwrap();

    let txs = TransactionRow::find_all(&conn, None, 1000).unwrap();
    let tx = txs.iter().find(|t| t.id == "TX-FUTURE-TARGET").unwrap();
    assert_eq!(tx.category.as_deref(), Some("Shopping"));
}
