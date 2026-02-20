//! Integration tests for recurring transaction detection.

mod common;

use pint::db::models::RecurringPattern;

#[test]
fn test_detect_recurring_patterns() {
    let conn = common::setup_test_db();

    let patterns = RecurringPattern::detect(&conn).unwrap();

    // Should detect several recurring patterns
    assert!(patterns.len() >= 5, "Should detect at least 5 recurring patterns, found {}", patterns.len());
}

#[test]
fn test_monthly_mortgage_detected() {
    let conn = common::setup_test_db();

    let patterns = RecurringPattern::detect(&conn).unwrap();

    // Find mortgage pattern
    let mortgage = patterns.iter()
        .find(|p| p.merchant.to_lowercase().contains("wells fargo"));

    assert!(mortgage.is_some(), "Should detect mortgage as recurring");
    let mortgage = mortgage.unwrap();

    assert_eq!(mortgage.frequency, "monthly");
    assert_eq!(mortgage.occurrences, 6);
    assert!(mortgage.avg_amount < 0.0, "Mortgage should be an expense");
    assert_eq!(mortgage.category.as_deref(), Some("Mortgage"));
}

#[test]
fn test_monthly_utilities_detected() {
    let conn = common::setup_test_db();

    let patterns = RecurringPattern::detect(&conn).unwrap();

    // Find electric bill pattern
    let electric = patterns.iter()
        .find(|p| p.merchant.to_lowercase().contains("pacific gas"));

    assert!(electric.is_some(), "Should detect electric bill as recurring");
    let electric = electric.unwrap();

    assert_eq!(electric.frequency, "monthly");
    assert_eq!(electric.occurrences, 6);
    assert_eq!(electric.category.as_deref(), Some("Utilities"));
}

#[test]
fn test_bimonthly_insurance_detected() {
    let conn = common::setup_test_db();

    let patterns = RecurringPattern::detect(&conn).unwrap();

    // Find car insurance pattern (bi-monthly)
    let insurance = patterns.iter()
        .find(|p| p.merchant.to_lowercase().contains("geico"));

    assert!(insurance.is_some(), "Should detect car insurance as recurring");
    let insurance = insurance.unwrap();

    assert_eq!(insurance.frequency, "bi-monthly");
    assert_eq!(insurance.occurrences, 3);
    assert_eq!(insurance.category.as_deref(), Some("Insurance"));
}

#[test]
fn test_subscription_detected() {
    let conn = common::setup_test_db();

    let patterns = RecurringPattern::detect(&conn).unwrap();

    // Find Netflix subscription
    let netflix = patterns.iter()
        .find(|p| p.merchant.to_lowercase().contains("netflix"));

    assert!(netflix.is_some(), "Should detect Netflix as recurring");
    let netflix = netflix.unwrap();

    assert_eq!(netflix.frequency, "monthly");
    assert_eq!(netflix.occurrences, 6);
    assert!(netflix.avg_amount.abs() - 15.99 < 0.01, "Netflix should be ~$15.99");
}

#[test]
fn test_gym_with_description_variations_detected() {
    let conn = common::setup_test_db();

    let patterns = RecurringPattern::detect(&conn).unwrap();

    // The gym membership has various description suffixes (~ Monthly, ~ Auto Pay, etc.)
    // Should still be detected as one recurring pattern due to normalization
    let gym = patterns.iter()
        .find(|p| p.merchant.to_lowercase().contains("planet fitness"));

    assert!(gym.is_some(), "Should detect gym membership despite description variations");
    let gym = gym.unwrap();

    assert_eq!(gym.frequency, "monthly");
    assert_eq!(gym.occurrences, 6);
}

#[test]
fn test_income_excluded() {
    let conn = common::setup_test_db();

    let patterns = RecurringPattern::detect(&conn).unwrap();

    // Paycheck (Income category) should be excluded
    let paycheck = patterns.iter()
        .find(|p| p.merchant.to_lowercase().contains("acme corp"));

    assert!(paycheck.is_none(), "Income transactions should be excluded from recurring");
}

#[test]
fn test_transfers_excluded() {
    let conn = common::setup_test_db();

    let patterns = RecurringPattern::detect(&conn).unwrap();

    // Transfer to savings (Transfers category) should be excluded
    let transfer = patterns.iter()
        .find(|p| p.merchant.to_lowercase().contains("transfer"));

    assert!(transfer.is_none(), "Transfer transactions should be excluded from recurring");
}

#[test]
fn test_irregular_transactions_not_detected() {
    let conn = common::setup_test_db();

    let patterns = RecurringPattern::detect(&conn).unwrap();

    // Grocery shopping is irregular, should not be detected
    let groceries = patterns.iter()
        .find(|p| p.merchant.to_lowercase().contains("whole foods")
                || p.merchant.to_lowercase().contains("trader joes")
                || p.merchant.to_lowercase().contains("safeway"));

    assert!(groceries.is_none(), "Irregular grocery shopping should not be detected as recurring");
}

#[test]
fn test_recurring_sorted_by_date() {
    let conn = common::setup_test_db();

    let patterns = RecurringPattern::detect(&conn).unwrap();

    // Should be sorted by last_date descending
    for i in 0..patterns.len() - 1 {
        assert!(patterns[i].last_date >= patterns[i + 1].last_date,
            "Recurring patterns should be sorted by date descending");
    }
}

#[test]
fn test_recurring_requires_minimum_occurrences() {
    let conn = common::setup_test_db();

    let patterns = RecurringPattern::detect(&conn).unwrap();

    // All detected patterns should have at least 3 occurrences
    for pattern in &patterns {
        assert!(pattern.occurrences >= 3,
            "Pattern '{}' has only {} occurrences, minimum is 3",
            pattern.merchant, pattern.occurrences);
    }
}

#[test]
fn test_recurring_has_category_when_available() {
    let conn = common::setup_test_db();

    let patterns = RecurringPattern::detect(&conn).unwrap();

    // Most patterns should have categories since our synthetic data is categorized
    let categorized = patterns.iter().filter(|p| p.category.is_some()).count();

    assert!(categorized > patterns.len() / 2,
        "Most recurring patterns should have categories");
}

#[test]
fn test_recurring_average_amount_calculation() {
    let conn = common::setup_test_db();

    let patterns = RecurringPattern::detect(&conn).unwrap();

    // Find Comcast (internet) - consistent $79.99
    let internet = patterns.iter()
        .find(|p| p.merchant.to_lowercase().contains("comcast"));

    assert!(internet.is_some());
    let internet = internet.unwrap();

    // Should be exactly -79.99 since all amounts are the same
    assert!((internet.avg_amount - (-79.99)).abs() < 0.01,
        "Comcast average should be -$79.99, got {}", internet.avg_amount);
}

#[test]
fn test_recurring_with_varying_amounts() {
    let conn = common::setup_test_db();

    let patterns = RecurringPattern::detect(&conn).unwrap();

    // Electric bill has varying amounts
    let electric = patterns.iter()
        .find(|p| p.merchant.to_lowercase().contains("pacific gas"));

    assert!(electric.is_some());
    let electric = electric.unwrap();

    // Average should be around -$130 (varies between $118 and $145)
    assert!(electric.avg_amount < -100.0 && electric.avg_amount > -150.0,
        "Electric bill average should be around -$130, got {}", electric.avg_amount);
}
