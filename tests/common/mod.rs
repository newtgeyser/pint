//! Test helpers and synthetic data generators for integration tests.

use chrono::{Duration, TimeZone, Utc};
use rusqlite::Connection;

/// Set up a test database with rich synthetic data representing a realistic
/// personal finance scenario with multiple accounts, recurring transactions,
/// categorized expenses, holdings, and assets.
pub fn setup_test_db() -> Connection {
    let conn = pint::db::open_in_memory().expect("Failed to create test database");
    populate_synthetic_data(&conn);
    conn
}

/// Populate the database with comprehensive synthetic data.
fn populate_synthetic_data(conn: &Connection) {
    let now = Utc::now().timestamp();

    // ========== CATEGORIES ==========
    let categories = [
        "Income",
        "Transfers",
        "Investments",
        "Groceries",
        "Utilities",
        "Mortgage",
        "Insurance",
        "Subscriptions",
        "Dining",
        "Transportation",
        "Entertainment",
        "Healthcare",
        "Shopping",
    ];

    for cat in &categories {
        conn.execute(
            "INSERT INTO categories (name, created_at) VALUES (?1, ?2)",
            rusqlite::params![cat, now],
        )
        .unwrap();
    }

    // Helper to get category ID
    let cat_id = |name: &str| -> i64 {
        conn.query_row("SELECT id FROM categories WHERE name = ?1", [name], |row| {
            row.get(0)
        })
        .unwrap()
    };

    // ========== ACCOUNTS ==========
    // Main checking account
    conn.execute(
        "INSERT INTO accounts (id, name, nickname, institution, account_type, balance, balance_date, currency, manual, created_at, updated_at)
         VALUES ('ACT-001-CHECKING', 'Primary Checking Account', 'Main Checking', 'First National Bank', 'checking', 1250000, ?1, 'USD', 0, ?1, ?1)",
        [now],
    ).unwrap();

    // Savings account
    conn.execute(
        "INSERT INTO accounts (id, name, nickname, institution, account_type, balance, balance_date, currency, manual, created_at, updated_at)
         VALUES ('ACT-002-SAVINGS', 'High Yield Savings', 'Emergency Fund', 'First National Bank', 'savings', 2500000, ?1, 'USD', 0, ?1, ?1)",
        [now],
    ).unwrap();

    // Credit card
    conn.execute(
        "INSERT INTO accounts (id, name, nickname, institution, account_type, balance, balance_date, currency, manual, created_at, updated_at)
         VALUES ('ACT-003-CREDIT', 'Rewards Credit Card', 'Chase Card', 'Chase Bank', 'credit', -185000, ?1, 'USD', 0, ?1, ?1)",
        [now],
    ).unwrap();

    // Brokerage account
    conn.execute(
        "INSERT INTO accounts (id, name, nickname, institution, account_type, balance, balance_date, currency, manual, created_at, updated_at)
         VALUES ('ACT-004-BROKERAGE', 'Individual Brokerage', 'Taxable Investments', 'Vanguard', 'brokerage', 15000000, ?1, 'USD', 0, ?1, ?1)",
        [now],
    ).unwrap();

    // Retirement account (401k)
    conn.execute(
        "INSERT INTO accounts (id, name, nickname, institution, account_type, balance, balance_date, currency, manual, created_at, updated_at)
         VALUES ('ACT-005-401K', '401k Retirement Plan', '401k', 'Fidelity', 'retirement', 45000000, ?1, 'USD', 0, ?1, ?1)",
        [now],
    ).unwrap();

    // Manual account (no transactions)
    conn.execute(
        "INSERT INTO accounts (id, name, nickname, institution, account_type, balance, balance_date, currency, manual, created_at, updated_at)
         VALUES ('ACT-006-MANUAL', 'Cash Stash', NULL, NULL, 'checking', 50000, ?1, 'USD', 1, ?1, ?1)",
        [now],
    ).unwrap();

    // ========== TRANSACTIONS ==========
    // We'll create transactions going back 6 months with various patterns

    let base_date = Utc::now() - Duration::days(180);

    // --- RECURRING MONTHLY TRANSACTIONS (for recurring detection) ---

    // Mortgage payment - 1st of each month, consistent amount
    for i in 0..6 {
        let date = base_date + Duration::days(i * 30 + 1);
        let tx_id = format!("TX-MORTGAGE-{}", i);
        conn.execute(
            "INSERT INTO transactions (id, account_id, posted, amount, description, pending, category_id, created_at, updated_at)
             VALUES (?1, 'ACT-001-CHECKING', ?2, -245000, 'WELLS FARGO MORTGAGE AUTOPAY', 0, ?3, ?4, ?4)",
            rusqlite::params![tx_id, date.timestamp(), cat_id("Mortgage"), now],
        ).unwrap();
    }

    // Electric bill - around 15th, slight variation in amount
    let electric_amounts = [-12500, -13200, -11800, -14500, -12900, -13100];
    for (i, amount) in electric_amounts.iter().enumerate() {
        let date = base_date + Duration::days(i as i64 * 30 + 15);
        let tx_id = format!("TX-ELECTRIC-{}", i);
        conn.execute(
            "INSERT INTO transactions (id, account_id, posted, amount, description, pending, category_id, created_at, updated_at)
             VALUES (?1, 'ACT-001-CHECKING', ?2, ?3, 'PACIFIC GAS ELECTRIC~ Online Payment', 0, ?4, ?5, ?5)",
            rusqlite::params![tx_id, date.timestamp(), amount, cat_id("Utilities"), now],
        ).unwrap();
    }

    // Internet bill - around 20th, consistent
    for i in 0..6 {
        let date = base_date + Duration::days(i * 30 + 20);
        let tx_id = format!("TX-INTERNET-{}", i);
        conn.execute(
            "INSERT INTO transactions (id, account_id, posted, amount, description, pending, category_id, created_at, updated_at)
             VALUES (?1, 'ACT-001-CHECKING', ?2, -7999, 'COMCAST CABLE COMM', 0, ?3, ?4, ?4)",
            rusqlite::params![tx_id, date.timestamp(), cat_id("Utilities"), now],
        ).unwrap();
    }

    // Car insurance - bi-monthly (every 2 months)
    for i in 0..3 {
        let date = base_date + Duration::days(i * 60 + 5);
        let tx_id = format!("TX-CARINS-{}", i);
        conn.execute(
            "INSERT INTO transactions (id, account_id, posted, amount, description, pending, category_id, created_at, updated_at)
             VALUES (?1, 'ACT-001-CHECKING', ?2, -28500, 'GEICO AUTO INSURANCE', 0, ?3, ?4, ?4)",
            rusqlite::params![tx_id, date.timestamp(), cat_id("Insurance"), now],
        ).unwrap();
    }

    // Netflix subscription - monthly on credit card
    for i in 0..6 {
        let date = base_date + Duration::days(i * 30 + 8);
        let tx_id = format!("TX-NETFLIX-{}", i);
        conn.execute(
            "INSERT INTO transactions (id, account_id, posted, amount, description, pending, category_id, created_at, updated_at)
             VALUES (?1, 'ACT-003-CREDIT', ?2, -1599, 'NETFLIX.COM', 0, ?3, ?4, ?4)",
            rusqlite::params![tx_id, date.timestamp(), cat_id("Subscriptions"), now],
        ).unwrap();
    }

    // Spotify subscription - monthly on credit card
    for i in 0..6 {
        let date = base_date + Duration::days(i * 30 + 12);
        let tx_id = format!("TX-SPOTIFY-{}", i);
        conn.execute(
            "INSERT INTO transactions (id, account_id, posted, amount, description, pending, category_id, created_at, updated_at)
             VALUES (?1, 'ACT-003-CREDIT', ?2, -1099, 'SPOTIFY USA', 0, ?3, ?4, ?4)",
            rusqlite::params![tx_id, date.timestamp(), cat_id("Subscriptions"), now],
        ).unwrap();
    }

    // Gym membership - monthly, with description variation (tests normalization)
    let gym_descs = [
        "PLANET FITNESS~ Monthly",
        "PLANET FITNESS",
        "PLANET FITNESS~ Auto Pay",
        "PLANET FITNESS~ Monthly Due",
        "PLANET FITNESS",
        "PLANET FITNESS~ Membership",
    ];
    for (i, desc) in gym_descs.iter().enumerate() {
        let date = base_date + Duration::days(i as i64 * 30 + 3);
        let tx_id = format!("TX-GYM-{}", i);
        conn.execute(
            "INSERT INTO transactions (id, account_id, posted, amount, description, pending, category_id, created_at, updated_at)
             VALUES (?1, 'ACT-003-CREDIT', ?2, -2500, ?3, 0, ?4, ?5, ?5)",
            rusqlite::params![tx_id, date.timestamp(), desc, cat_id("Healthcare"), now],
        ).unwrap();
    }

    // --- INCOME (monthly paycheck) - should be excluded from recurring expenses ---
    for i in 0..6 {
        let date = base_date + Duration::days(i * 30 + 1);
        let tx_id = format!("TX-PAYCHECK-{}", i);
        conn.execute(
            "INSERT INTO transactions (id, account_id, posted, amount, description, pending, category_id, created_at, updated_at)
             VALUES (?1, 'ACT-001-CHECKING', ?2, 750000, 'ACME CORP DIRECT DEPOSIT PAYROLL', 0, ?3, ?4, ?4)",
            rusqlite::params![tx_id, date.timestamp(), cat_id("Income"), now],
        ).unwrap();
    }

    // --- TRANSFERS (should be excluded from recurring) ---
    for i in 0..4 {
        let date = base_date + Duration::days(i * 30 + 2);
        let tx_id = format!("TX-TRANSFER-{}", i);
        conn.execute(
            "INSERT INTO transactions (id, account_id, posted, amount, description, pending, category_id, created_at, updated_at)
             VALUES (?1, 'ACT-001-CHECKING', ?2, -50000, 'TRANSFER TO SAVINGS', 0, ?3, ?4, ?4)",
            rusqlite::params![tx_id, date.timestamp(), cat_id("Transfers"), now],
        ).unwrap();
    }

    // --- IRREGULAR TRANSACTIONS (should NOT be detected as recurring) ---

    // Random grocery shopping (various amounts, various days)
    let grocery_data = [
        (5, -8523, "WHOLE FOODS MARKET #1234"),
        (12, -4521, "TRADER JOES #567"),
        (18, -6789, "SAFEWAY STORE #890"),
        (25, -3456, "WHOLE FOODS MARKET #1234"),
        (38, -9012, "COSTCO WHSE #123"),
        (45, -5678, "TRADER JOES #567"),
        (52, -7890, "SAFEWAY STORE #890"),
        (67, -4123, "WHOLE FOODS MARKET #1234"),
        (75, -6543, "COSTCO WHSE #123"),
        (89, -8765, "TRADER JOES #567"),
        (102, -3210, "SAFEWAY STORE #890"),
        (115, -5432, "WHOLE FOODS MARKET #1234"),
        (128, -7654, "COSTCO WHSE #123"),
        (140, -2345, "TRADER JOES #567"),
        (155, -4567, "SAFEWAY STORE #890"),
    ];
    for (i, (days, amount, desc)) in grocery_data.iter().enumerate() {
        let date = base_date + Duration::days(*days);
        let tx_id = format!("TX-GROCERY-{}", i);
        conn.execute(
            "INSERT INTO transactions (id, account_id, posted, amount, description, pending, category_id, created_at, updated_at)
             VALUES (?1, 'ACT-003-CREDIT', ?2, ?3, ?4, 0, ?5, ?6, ?6)",
            rusqlite::params![tx_id, date.timestamp(), amount, desc, cat_id("Groceries"), now],
        ).unwrap();
    }

    // Random dining
    let dining_data = [
        (7, -4500, "CHIPOTLE ONLINE"),
        (14, -3200, "STARBUCKS STORE #12345"),
        (21, -8900, "THE FANCY RESTAURANT"),
        (35, -2100, "MCDONALDS F12345"),
        (48, -5600, "OLIVE GARDEN #789"),
        (63, -1800, "STARBUCKS STORE #12345"),
        (78, -6700, "SUSHI PALACE"),
        (92, -2900, "CHIPOTLE ONLINE"),
        (108, -4200, "TACO BELL #456"),
        (125, -7800, "STEAKHOUSE PRIME"),
        (142, -1500, "STARBUCKS STORE #12345"),
        (160, -3800, "PIZZA HUT #789"),
    ];
    for (i, (days, amount, desc)) in dining_data.iter().enumerate() {
        let date = base_date + Duration::days(*days);
        let tx_id = format!("TX-DINING-{}", i);
        conn.execute(
            "INSERT INTO transactions (id, account_id, posted, amount, description, pending, category_id, created_at, updated_at)
             VALUES (?1, 'ACT-003-CREDIT', ?2, ?3, ?4, 0, ?5, ?6, ?6)",
            rusqlite::params![tx_id, date.timestamp(), amount, desc, cat_id("Dining"), now],
        ).unwrap();
    }

    // Gas/Transportation
    let gas_data = [
        (3, -4500, "SHELL OIL 12345678"),
        (19, -5200, "CHEVRON 0012345"),
        (36, -4800, "SHELL OIL 12345678"),
        (55, -5100, "76 GAS STATION"),
        (72, -4600, "CHEVRON 0012345"),
        (91, -5300, "SHELL OIL 12345678"),
        (112, -4900, "76 GAS STATION"),
        (133, -5000, "CHEVRON 0012345"),
        (156, -4700, "SHELL OIL 12345678"),
    ];
    for (i, (days, amount, desc)) in gas_data.iter().enumerate() {
        let date = base_date + Duration::days(*days);
        let tx_id = format!("TX-GAS-{}", i);
        conn.execute(
            "INSERT INTO transactions (id, account_id, posted, amount, description, pending, category_id, created_at, updated_at)
             VALUES (?1, 'ACT-003-CREDIT', ?2, ?3, ?4, 0, ?5, ?6, ?6)",
            rusqlite::params![tx_id, date.timestamp(), amount, desc, cat_id("Transportation"), now],
        ).unwrap();
    }

    // Some uncategorized transactions
    let uncategorized_data = [
        (10, -2500, "AMAZON MKTPLACE PMTS"),
        (28, -15000, "AMAZON MKTPLACE PMTS"),
        (45, -8900, "AMAZON MKTPLACE PMTS"),
        (68, -3200, "UNKNOWN MERCHANT XYZ"),
        (95, -1800, "ATM WITHDRAWAL"),
        (120, -4500, "VENMO PAYMENT"),
        (145, -6700, "PAYPAL *EBAYSELLER"),
    ];
    for (i, (days, amount, desc)) in uncategorized_data.iter().enumerate() {
        let date = base_date + Duration::days(*days);
        let tx_id = format!("TX-UNCAT-{}", i);
        conn.execute(
            "INSERT INTO transactions (id, account_id, posted, amount, description, pending, category_id, created_at, updated_at)
             VALUES (?1, 'ACT-003-CREDIT', ?2, ?3, ?4, 0, NULL, ?5, ?5)",
            rusqlite::params![tx_id, date.timestamp(), amount, desc, now],
        ).unwrap();
    }

    // One pending transaction
    conn.execute(
        "INSERT INTO transactions (id, account_id, posted, amount, description, pending, category_id, created_at, updated_at)
         VALUES ('TX-PENDING-001', 'ACT-003-CREDIT', ?1, -5999, 'PENDING PURCHASE', 1, NULL, ?1, ?1)",
        [now],
    ).unwrap();

    // ========== MERCHANT RULES ==========
    let rules = [
        ("NETFLIX", "Subscriptions", "substring"),
        ("SPOTIFY", "Subscriptions", "substring"),
        ("WHOLE FOODS", "Groceries", "substring"),
        ("TRADER JOES", "Groceries", "substring"),
        ("SAFEWAY", "Groceries", "substring"),
        ("COSTCO", "Groceries", "substring"),
        ("STARBUCKS", "Dining", "substring"),
        ("CHIPOTLE", "Dining", "substring"),
        ("SHELL OIL", "Transportation", "substring"),
        ("CHEVRON", "Transportation", "substring"),
        ("GEICO", "Insurance", "token"),
    ];
    for (pattern, category, match_mode) in &rules {
        conn.execute(
            "INSERT INTO merchant_rules (pattern, match_mode, category_id, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![pattern.to_uppercase(), match_mode, cat_id(category), now],
        )
        .unwrap();
    }

    // ========== HOLDINGS ==========
    // Brokerage holdings
    let brokerage_holdings = [
        (
            "HOLD-001",
            "ACT-004-BROKERAGE",
            "VTI",
            "Vanguard Total Stock Market ETF",
            "50.000",
            22500,
            1000000,
            1125000,
        ),
        (
            "HOLD-002",
            "ACT-004-BROKERAGE",
            "VXUS",
            "Vanguard Total International Stock ETF",
            "75.000",
            5800,
            400000,
            435000,
        ),
        (
            "HOLD-003",
            "ACT-004-BROKERAGE",
            "BND",
            "Vanguard Total Bond Market ETF",
            "100.000",
            7200,
            700000,
            720000,
        ),
        (
            "HOLD-004",
            "ACT-004-BROKERAGE",
            "AAPL",
            "Apple Inc.",
            "25.000",
            17500,
            350000,
            437500,
        ),
    ];
    for (id, account, symbol, desc, shares, price, cost, value) in &brokerage_holdings {
        conn.execute(
            "INSERT INTO holdings (id, account_id, symbol, description, shares, price, cost_basis, market_value, currency, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'USD', ?9, ?9)",
            rusqlite::params![id, account, symbol, desc, shares, price, cost, value, now],
        ).unwrap();
    }

    // 401k holdings
    let retirement_holdings = [
        (
            "HOLD-101",
            "ACT-005-401K",
            "FXAIX",
            "Fidelity 500 Index Fund",
            "150.000",
            18000,
            2200000,
            2700000,
        ),
        (
            "HOLD-102",
            "ACT-005-401K",
            "FXNAX",
            "Fidelity US Bond Index Fund",
            "200.000",
            1050,
            200000,
            210000,
        ),
        (
            "HOLD-103",
            "ACT-005-401K",
            "FSPSX",
            "Fidelity International Index Fund",
            "100.000",
            4500,
            400000,
            450000,
        ),
    ];
    for (id, account, symbol, desc, shares, price, cost, value) in &retirement_holdings {
        conn.execute(
            "INSERT INTO holdings (id, account_id, symbol, description, shares, price, cost_basis, market_value, currency, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'USD', ?9, ?9)",
            rusqlite::params![id, account, symbol, desc, shares, price, cost, value, now],
        ).unwrap();
    }

    // ========== ASSETS ==========
    let assets = [
        (
            "Primary Residence",
            "real_estate",
            "123 Main Street, Anytown USA",
            55000000,
            35000000,
        ),
        (
            "2020 Toyota Camry",
            "vehicle",
            "VIN: 1234567890",
            2200000,
            2800000,
        ),
        (
            "Art Collection",
            "collectible",
            "Various pieces",
            1500000,
            800000,
        ),
    ];
    for (name, asset_type, desc, value, cost) in &assets {
        conn.execute(
            "INSERT INTO assets (name, asset_type, description, value, cost_basis, currency, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'USD', ?6, ?6)",
            rusqlite::params![name, asset_type, desc, value, cost, now],
        ).unwrap();
    }
    // ========== REWARD POINTS ==========
    let reward_points: [(&str, i64, Option<&str>); 4] = [
        ("Chase Sapphire Reserve", 85432, Some("Ultimate Rewards")),
        ("Chase Freedom Unlimited", 12650, Some("Ultimate Rewards")),
        ("Chase Amazon Prime", 4280, Some("Cashback points")),
        ("Capital One Venture", 45000, Some("Venture Miles")),
    ];
    for (program, points, note) in &reward_points {
        conn.execute(
            "INSERT INTO rewards_points (program, points, note, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4)",
            rusqlite::params![program, points, note, now],
        )
        .unwrap();
    }
}

/// Get the count of records in a table.
pub fn count_records(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT COUNT(*) FROM {}", table), [], |row| {
        row.get(0)
    })
    .unwrap()
}

/// Get a category ID by name.
pub fn get_category_id(conn: &Connection, name: &str) -> Option<i64> {
    conn.query_row("SELECT id FROM categories WHERE name = ?1", [name], |row| {
        row.get(0)
    })
    .ok()
}
