//! Integration tests for recurring transaction detection.

mod common;

use pint::db::models::RecurringPattern;

fn insert_series(conn: &rusqlite::Connection, merchant: &str, dates: &[i64], amounts: &[i64]) {
    for (index, posted) in dates.iter().enumerate() {
        conn.execute(
            "INSERT INTO transactions
             (id, account_id, posted, amount, description, pending, created_at, updated_at)
             VALUES (?1, 'ACT-001-CHECKING', ?2, ?3, ?4, 0, ?2, ?2)",
            rusqlite::params![
                format!("TX-FORECAST-{merchant}-{index}"),
                posted,
                amounts[index % amounts.len()],
                merchant,
            ],
        )
        .unwrap();
    }
}

#[test]
fn test_detect_recurring_patterns() {
    let conn = common::setup_test_db();

    let patterns = RecurringPattern::detect(&conn).unwrap();

    // Should detect several recurring patterns
    assert!(
        patterns.len() >= 5,
        "Should detect at least 5 recurring patterns, found {}",
        patterns.len()
    );
}

#[test]
fn test_monthly_mortgage_detected() {
    let conn = common::setup_test_db();

    let patterns = RecurringPattern::detect(&conn).unwrap();

    // Find mortgage pattern
    let mortgage = patterns
        .iter()
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
    let electric = patterns
        .iter()
        .find(|p| p.merchant.to_lowercase().contains("pacific gas"));

    assert!(
        electric.is_some(),
        "Should detect electric bill as recurring"
    );
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
    let insurance = patterns
        .iter()
        .find(|p| p.merchant.to_lowercase().contains("geico"));

    assert!(
        insurance.is_some(),
        "Should detect car insurance as recurring"
    );
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
    let netflix = patterns
        .iter()
        .find(|p| p.merchant.to_lowercase().contains("netflix"));

    assert!(netflix.is_some(), "Should detect Netflix as recurring");
    let netflix = netflix.unwrap();

    assert_eq!(netflix.frequency, "monthly");
    assert_eq!(netflix.occurrences, 6);
    assert!(
        netflix.avg_amount.abs() - 15.99 < 0.01,
        "Netflix should be ~$15.99"
    );
}

#[test]
fn test_gym_with_description_variations_detected() {
    let conn = common::setup_test_db();

    let patterns = RecurringPattern::detect(&conn).unwrap();

    // The gym membership has various description suffixes (~ Monthly, ~ Auto Pay, etc.)
    // Should still be detected as one recurring pattern due to normalization
    let gym = patterns
        .iter()
        .find(|p| p.merchant.to_lowercase().contains("planet fitness"));

    assert!(
        gym.is_some(),
        "Should detect gym membership despite description variations"
    );
    let gym = gym.unwrap();

    assert_eq!(gym.frequency, "monthly");
    assert_eq!(gym.occurrences, 6);
}

#[test]
fn test_income_excluded() {
    let conn = common::setup_test_db();

    let patterns = RecurringPattern::detect(&conn).unwrap();

    // Paycheck (Income category) should be excluded
    let paycheck = patterns
        .iter()
        .find(|p| p.merchant.to_lowercase().contains("acme corp"));

    assert!(
        paycheck.is_none(),
        "Income transactions should be excluded from recurring"
    );
}

#[test]
fn test_transfers_excluded() {
    let conn = common::setup_test_db();

    let patterns = RecurringPattern::detect(&conn).unwrap();

    // Transfer to savings (Transfers category) should be excluded
    let transfer = patterns
        .iter()
        .find(|p| p.merchant.to_lowercase().contains("transfer"));

    assert!(
        transfer.is_none(),
        "Transfer transactions should be excluded from recurring"
    );
}

#[test]
fn test_irregular_transactions_not_detected() {
    let conn = common::setup_test_db();

    let patterns = RecurringPattern::detect(&conn).unwrap();

    // Grocery shopping is irregular, should not be detected
    let groceries = patterns.iter().find(|p| {
        p.merchant.to_lowercase().contains("whole foods")
            || p.merchant.to_lowercase().contains("trader joes")
            || p.merchant.to_lowercase().contains("safeway")
    });

    assert!(
        groceries.is_none(),
        "Irregular grocery shopping should not be detected as recurring"
    );
}

#[test]
fn test_recurring_sorted_by_date() {
    let conn = common::setup_test_db();

    let patterns = RecurringPattern::detect(&conn).unwrap();

    // Should be sorted by last_date descending
    for i in 0..patterns.len() - 1 {
        assert!(
            patterns[i].last_date >= patterns[i + 1].last_date,
            "Recurring patterns should be sorted by date descending"
        );
    }
}

#[test]
fn test_recurring_requires_minimum_occurrences() {
    let conn = common::setup_test_db();

    let patterns = RecurringPattern::detect(&conn).unwrap();

    // All detected patterns should have at least 3 occurrences
    for pattern in &patterns {
        assert!(
            pattern.occurrences >= 3,
            "Pattern '{}' has only {} occurrences, minimum is 3",
            pattern.merchant,
            pattern.occurrences
        );
    }
}

#[test]
fn test_recurring_has_category_when_available() {
    let conn = common::setup_test_db();

    let patterns = RecurringPattern::detect(&conn).unwrap();

    // Most patterns should have categories since our synthetic data is categorized
    let categorized = patterns.iter().filter(|p| p.category.is_some()).count();

    assert!(
        categorized > patterns.len() / 2,
        "Most recurring patterns should have categories"
    );
}

#[test]
fn test_recurring_average_amount_calculation() {
    let conn = common::setup_test_db();

    let patterns = RecurringPattern::detect(&conn).unwrap();

    // Find Comcast (internet) - consistent $79.99
    let internet = patterns
        .iter()
        .find(|p| p.merchant.to_lowercase().contains("comcast"));

    assert!(internet.is_some());
    let internet = internet.unwrap();

    // Should be exactly -79.99 since all amounts are the same
    assert!(
        (internet.avg_amount - (-79.99)).abs() < 0.01,
        "Comcast average should be -$79.99, got {}",
        internet.avg_amount
    );
}

#[test]
fn test_recurring_with_varying_amounts() {
    let conn = common::setup_test_db();

    let patterns = RecurringPattern::detect(&conn).unwrap();

    // Electric bill has varying amounts
    let electric = patterns
        .iter()
        .find(|p| p.merchant.to_lowercase().contains("pacific gas"));

    assert!(electric.is_some());
    let electric = electric.unwrap();

    // Average should be around -$130 (varies between $118 and $145)
    assert!(
        electric.avg_amount < -100.0 && electric.avg_amount > -150.0,
        "Electric bill average should be around -$130, got {}",
        electric.avg_amount
    );
}

#[test]
fn test_detects_all_supported_frequencies() {
    let conn = common::setup_test_db();
    let day = 86_400;
    let as_of = 2_000_000_000;
    let cases = [
        ("CADENCE WEEKLY", 7, "weekly"),
        ("CADENCE BIWEEKLY", 14, "biweekly"),
        ("CADENCE MONTHLY", 30, "monthly"),
        ("CADENCE BIMONTHLY", 61, "bi-monthly"),
        ("CADENCE QUARTERLY", 91, "quarterly"),
        ("CADENCE ANNUAL", 365, "annual"),
    ];

    for (merchant, interval, _) in cases {
        let last = as_of - interval * day;
        insert_series(
            &conn,
            merchant,
            &[last - 2 * interval * day, last - interval * day, last],
            &[-1_000],
        );
    }

    let patterns = RecurringPattern::detect_at(&conn, as_of).unwrap();
    for (merchant, _, expected) in cases {
        let pattern = patterns
            .iter()
            .find(|pattern| pattern.merchant.to_uppercase().contains(merchant))
            .unwrap_or_else(|| panic!("missing {merchant}"));
        assert_eq!(pattern.frequency, expected);
        assert_eq!(pattern.status, "active");
    }
}

#[test]
fn test_forecast_status_variance_and_monthly_commitment() {
    let conn = common::setup_test_db();
    let day = 86_400;
    let last = 1_900_000_000;
    insert_series(
        &conn,
        "FORECAST VARIABLE SERVICE",
        &[last - 60 * day, last - 30 * day, last],
        &[-900, -1_000, -1_100],
    );

    let active = RecurringPattern::detect_at(&conn, last + 35 * day).unwrap();
    let active = active
        .iter()
        .find(|pattern| pattern.merchant.contains("Forecast variable"))
        .unwrap();
    assert_eq!(active.status, "active");
    assert!((active.monthly_commitment - 10.1458).abs() < 0.01);
    assert!((active.amount_variance - 0.8165).abs() < 0.01);
    assert!(!active.expected_next_date.is_empty());

    let overdue = RecurringPattern::detect_at(&conn, last + 40 * day).unwrap();
    assert_eq!(
        overdue
            .iter()
            .find(|pattern| pattern.merchant.contains("Forecast variable"))
            .unwrap()
            .status,
        "overdue"
    );

    let inactive = RecurringPattern::detect_at(&conn, last + 61 * day).unwrap();
    assert_eq!(
        inactive
            .iter()
            .find(|pattern| pattern.merchant.contains("Forecast variable"))
            .unwrap()
            .status,
        "inactive"
    );
}
