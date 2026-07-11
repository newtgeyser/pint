//! Integration tests for rules and categorization functionality.

mod common;

use pint::db::models::{Category, MerchantRule, RuleRow, Transaction, TransactionRow};

#[test]
fn test_list_all_categories() {
    let conn = common::setup_test_db();

    let categories = Category::find_all(&conn).unwrap();

    // Should have 13 categories
    assert_eq!(categories.len(), 13);
}

#[test]
fn test_categories_sorted_by_name() {
    let conn = common::setup_test_db();

    let categories = Category::find_all(&conn).unwrap();

    // Should be sorted alphabetically
    for i in 0..categories.len() - 1 {
        assert!(
            categories[i].name <= categories[i + 1].name,
            "Categories should be sorted alphabetically"
        );
    }
}

#[test]
fn test_categories_for_select() {
    let conn = common::setup_test_db();

    let categories = Category::find_all_for_select(&conn).unwrap();

    assert_eq!(categories.len(), 13);

    // Each tuple should have (id, name)
    for (id, name) in &categories {
        assert!(*id > 0);
        assert!(!name.is_empty());
    }
}

#[test]
fn test_categories_for_select_strings() {
    let conn = common::setup_test_db();

    let categories = Category::find_all_for_select_strings(&conn).unwrap();

    assert_eq!(categories.len(), 13);

    // Each tuple should have (id_string, name)
    for (id_str, name) in &categories {
        let id: i64 = id_str.parse().expect("ID should be parseable as i64");
        assert!(id > 0);
        assert!(!name.is_empty());
    }
}

#[test]
fn test_list_all_rules() {
    let conn = common::setup_test_db();

    let rules = RuleRow::find_all(&conn).unwrap();

    // Should have 11 rules from synthetic data
    assert_eq!(rules.len(), 11);
}

#[test]
fn test_rules_sorted_by_pattern() {
    let conn = common::setup_test_db();

    let rules = RuleRow::find_all(&conn).unwrap();

    // Should be sorted alphabetically by pattern
    for i in 0..rules.len() - 1 {
        assert!(
            rules[i].pattern <= rules[i + 1].pattern,
            "Rules should be sorted by pattern: {} vs {}",
            rules[i].pattern,
            rules[i + 1].pattern
        );
    }
}

#[test]
fn test_rule_has_category() {
    let conn = common::setup_test_db();

    let rules = RuleRow::find_all(&conn).unwrap();

    // All rules should have categories
    for rule in &rules {
        assert!(!rule.category.is_empty());
    }
}

#[test]
fn test_substring_rule_match() {
    let conn = common::setup_test_db();

    // NETFLIX rule should match "NETFLIX.COM"
    let rule = RuleRow::find_for_description(&conn, "NETFLIX.COM").unwrap();

    assert!(rule.is_some());
    let rule = rule.unwrap();
    assert_eq!(rule.pattern, "NETFLIX");
    assert_eq!(rule.match_mode, "substring");
    assert_eq!(rule.category, "Subscriptions");
}

#[test]
fn test_substring_rule_case_insensitive() {
    let conn = common::setup_test_db();

    // Should match regardless of case
    let rule = RuleRow::find_for_description(&conn, "netflix streaming").unwrap();

    assert!(rule.is_some());
    assert_eq!(rule.unwrap().category, "Subscriptions");
}

#[test]
fn test_token_rule_match() {
    let conn = common::setup_test_db();

    // GEICO uses token matching, should match "GEICO AUTO INSURANCE"
    let rule = RuleRow::find_for_description(&conn, "GEICO AUTO INSURANCE").unwrap();

    assert!(rule.is_some());
    let rule = rule.unwrap();
    assert_eq!(rule.pattern, "GEICO");
    assert_eq!(rule.match_mode, "token");
    assert_eq!(rule.category, "Insurance");
}

#[test]
fn test_token_rule_no_match_substring() {
    let conn = common::setup_test_db();

    // Token rule for GEICO should NOT match "SOMEGEICO" (not a word boundary)
    let rule = RuleRow::find_for_description(&conn, "SOMEGEICO INSURANCE").unwrap();

    // Should not match GEICO rule
    let matches_geico = rule.as_ref().map(|r| r.pattern == "GEICO").unwrap_or(false);
    assert!(!matches_geico, "Token rule should not match as substring");
}

#[test]
fn test_no_rule_match() {
    let conn = common::setup_test_db();

    let rule = RuleRow::find_for_description(&conn, "COMPLETELY UNKNOWN MERCHANT").unwrap();

    assert!(rule.is_none());
}

#[test]
fn test_add_rule() {
    let conn = common::setup_test_db();

    let initial_count = RuleRow::find_all(&conn).unwrap().len();

    // Get a category ID
    let category_id = common::get_category_id(&conn, "Shopping").unwrap();

    // Add a new rule
    MerchantRule::upsert(&conn, "TARGET", "substring", category_id).unwrap();

    let rules = RuleRow::find_all(&conn).unwrap();
    assert_eq!(rules.len(), initial_count + 1);

    // Verify the new rule exists and matches
    let rule = RuleRow::find_for_description(&conn, "TARGET STORE #123").unwrap();
    assert!(rule.is_some());
    assert_eq!(rule.unwrap().category, "Shopping");
}

#[test]
fn test_update_rule_category() {
    let conn = common::setup_test_db();

    // Get a different category ID
    let new_category_id = common::get_category_id(&conn, "Entertainment").unwrap();

    // Update NETFLIX rule to use Entertainment instead of Subscriptions
    MerchantRule::upsert(&conn, "NETFLIX", "substring", new_category_id).unwrap();

    // Verify the change
    let rule = RuleRow::find_for_description(&conn, "NETFLIX.COM")
        .unwrap()
        .unwrap();
    assert_eq!(rule.category, "Entertainment");
}

#[test]
fn test_delete_rule() {
    let conn = common::setup_test_db();

    let initial_count = RuleRow::find_all(&conn).unwrap().len();

    // Delete the NETFLIX rule
    MerchantRule::delete_by_pattern(&conn, "NETFLIX").unwrap();

    let rules = RuleRow::find_all(&conn).unwrap();
    assert_eq!(rules.len(), initial_count - 1);

    // Verify the rule no longer matches
    let rule = RuleRow::find_for_description(&conn, "NETFLIX.COM").unwrap();
    assert!(rule.is_none() || rule.unwrap().pattern != "NETFLIX");
}

#[test]
fn test_apply_rule_to_transactions() {
    let conn = common::setup_test_db();

    // First, clear category from Amazon transactions
    conn.execute(
        "UPDATE transactions SET category_id = NULL WHERE description LIKE '%AMAZON%'",
        [],
    )
    .unwrap();

    // Verify they're now uncategorized
    let amazon_txs = TransactionRow::find_all(&conn, None, 1000).unwrap();
    let uncategorized_amazon: Vec<_> = amazon_txs
        .iter()
        .filter(|t| t.description.contains("AMAZON") && t.category.is_none())
        .collect();
    assert!(
        uncategorized_amazon.len() > 0,
        "Should have uncategorized Amazon transactions"
    );

    // Get Shopping category ID
    let shopping_id = common::get_category_id(&conn, "Shopping").unwrap();

    // Apply new rule for Amazon
    let count =
        MerchantRule::apply_to_transactions(&conn, "AMAZON", "substring", shopping_id).unwrap();
    assert!(count > 0, "Should have updated some transactions");

    // Verify the transactions are now categorized
    let amazon_txs = TransactionRow::find_all(&conn, None, 1000).unwrap();
    let categorized_amazon: Vec<_> = amazon_txs
        .iter()
        .filter(|t| t.description.contains("AMAZON") && t.category.as_deref() == Some("Shopping"))
        .collect();
    assert!(
        categorized_amazon.len() > 0,
        "Amazon transactions should now be categorized"
    );
}

#[test]
fn test_apply_token_rule_to_transactions() {
    let conn = common::setup_test_db();

    // Add a transaction that should match token rule
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT INTO transactions (id, account_id, posted, amount, description, pending, category_id, created_at, updated_at)
         VALUES ('TX-TEST-TOKEN', 'ACT-003-CREDIT', ?1, -5000, 'GEICO PAYMENT ONLINE', 0, NULL, ?1, ?1)",
        [now],
    ).unwrap();

    // Get Insurance category ID
    let insurance_id = common::get_category_id(&conn, "Insurance").unwrap();

    // Apply token rule
    let count = MerchantRule::apply_to_transactions(&conn, "GEICO", "token", insurance_id).unwrap();
    assert!(count >= 1, "Should have updated at least one transaction");

    // Verify the transaction is categorized
    let txs = TransactionRow::find_all(&conn, None, 1000).unwrap();
    let tx = txs.iter().find(|t| t.id == "TX-TEST-TOKEN").unwrap();
    assert_eq!(tx.category.as_deref(), Some("Insurance"));
}

#[test]
fn test_set_single_transaction_category() {
    let conn = common::setup_test_db();

    // Get an uncategorized transaction
    let tx_id = "TX-UNCAT-0";

    // Verify it's uncategorized
    let txs = TransactionRow::find_all(&conn, None, 1000).unwrap();
    let tx = txs.iter().find(|t| t.id == tx_id).unwrap();
    assert!(tx.category.is_none());

    // Set the category
    let entertainment_id = common::get_category_id(&conn, "Entertainment").unwrap();
    Transaction::set_category(&conn, tx_id, entertainment_id).unwrap();

    // Verify the change
    let txs = TransactionRow::find_all(&conn, None, 1000).unwrap();
    let tx = txs.iter().find(|t| t.id == tx_id).unwrap();
    assert_eq!(tx.category.as_deref(), Some("Entertainment"));
}

#[test]
fn test_add_new_category() {
    let conn = common::setup_test_db();

    let initial_count = Category::find_all(&conn).unwrap().len();

    // Add a new category
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT INTO categories (name, created_at) VALUES ('Pets', ?1)",
        [now],
    )
    .unwrap();

    let categories = Category::find_all(&conn).unwrap();
    assert_eq!(categories.len(), initial_count + 1);

    // Verify it's in the list
    let pet_cat = categories.iter().find(|c| c.name == "Pets");
    assert!(pet_cat.is_some());
}

#[test]
fn test_category_has_id() {
    let conn = common::setup_test_db();

    let categories = Category::find_all(&conn).unwrap();

    for cat in &categories {
        assert!(cat.id > 0);
    }
}

#[test]
fn test_multiple_rules_same_category() {
    let conn = common::setup_test_db();

    let rules = RuleRow::find_all(&conn).unwrap();

    // Multiple grocery rules should all point to Groceries
    let grocery_rules: Vec<_> = rules.iter().filter(|r| r.category == "Groceries").collect();

    // WHOLE FOODS, TRADER JOES, SAFEWAY, COSTCO
    assert_eq!(grocery_rules.len(), 4);

    // All should have the same category
    for rule in &grocery_rules {
        assert_eq!(rule.category, "Groceries");
    }
}

#[test]
fn test_rule_match_modes() {
    let conn = common::setup_test_db();

    let rules = RuleRow::find_all(&conn).unwrap();

    // Most rules use substring
    let substring_rules = rules.iter().filter(|r| r.match_mode == "substring").count();
    let token_rules = rules.iter().filter(|r| r.match_mode == "token").count();

    assert!(substring_rules > 0, "Should have substring rules");
    assert!(token_rules > 0, "Should have token rules");
    assert_eq!(
        substring_rules + token_rules,
        rules.len(),
        "All rules should be substring or token"
    );
}

#[test]
fn test_specific_rules_exist() {
    let conn = common::setup_test_db();

    let rules = RuleRow::find_all(&conn).unwrap();

    // Check specific expected rules
    let rule_patterns: Vec<&str> = rules.iter().map(|r| r.pattern.as_str()).collect();

    assert!(rule_patterns.contains(&"NETFLIX"));
    assert!(rule_patterns.contains(&"SPOTIFY"));
    assert!(rule_patterns.contains(&"WHOLE FOODS"));
    assert!(rule_patterns.contains(&"GEICO"));
    assert!(rule_patterns.contains(&"STARBUCKS"));
}
