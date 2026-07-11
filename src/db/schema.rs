use anyhow::Result;
use rusqlite::{Connection, OptionalExtension};

pub fn is_initialized(conn: &Connection) -> Result<bool> {
    let required_tables = [
        "accounts",
        "categories",
        "transactions",
        "merchant_rules",
        "config",
    ];

    for table in required_tables {
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [table],
            |row| row.get(0),
        )?;
        if !exists {
            return Ok(false);
        }
    }

    Ok(true)
}

pub fn create_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS accounts (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            nickname TEXT,
            institution TEXT,
            account_type TEXT NOT NULL DEFAULT 'unknown',
            balance INTEGER,
            balance_date INTEGER,
            currency TEXT DEFAULT 'USD',
            manual INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS categories (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            parent_id INTEGER REFERENCES categories(id),
            created_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS transactions (
            id TEXT PRIMARY KEY,
            account_id TEXT NOT NULL REFERENCES accounts(id),
            posted INTEGER NOT NULL,
            amount INTEGER NOT NULL,
            description TEXT NOT NULL,
            pending INTEGER DEFAULT 0,
            category_id INTEGER REFERENCES categories(id),
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_transactions_account
            ON transactions(account_id);
        CREATE INDEX IF NOT EXISTS idx_transactions_posted
            ON transactions(posted);

        CREATE TABLE IF NOT EXISTS merchant_rules (
            pattern TEXT PRIMARY KEY,
            match_mode TEXT NOT NULL DEFAULT 'substring',
            category_id INTEGER NOT NULL REFERENCES categories(id),
            created_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS config (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS holdings (
            id TEXT PRIMARY KEY,
            account_id TEXT NOT NULL REFERENCES accounts(id),
            symbol TEXT,
            description TEXT,
            shares TEXT NOT NULL,
            price INTEGER,
            cost_basis INTEGER,
            market_value INTEGER,
            currency TEXT DEFAULT 'USD',
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_holdings_account
            ON holdings(account_id);

        CREATE TABLE IF NOT EXISTS assets (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            asset_type TEXT NOT NULL,
            description TEXT,
            value INTEGER,
            cost_basis INTEGER,
            currency TEXT DEFAULT 'USD',
            acquired_date INTEGER,
            metadata TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS rewards_points (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            program TEXT NOT NULL UNIQUE,
            points INTEGER NOT NULL DEFAULT 0,
            note TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS reimbursers (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL COLLATE NOCASE UNIQUE,
            created_at INTEGER NOT NULL
        );
        ",
    )?;

    Ok(())
}

pub fn migrate(conn: &Connection) -> Result<()> {
    // Add category_id column if it doesn't exist (for existing databases)
    let has_category = conn
        .prepare("SELECT category_id FROM transactions LIMIT 0")
        .is_ok();

    if !has_category {
        conn.execute_batch(
            "ALTER TABLE transactions ADD COLUMN category_id INTEGER REFERENCES categories(id);",
        )?;
    }

    // Add match_mode column if it doesn't exist (for existing databases)
    let has_match_mode = conn
        .prepare("SELECT match_mode FROM merchant_rules LIMIT 0")
        .is_ok();
    if !has_match_mode {
        conn.execute_batch(
            "ALTER TABLE merchant_rules ADD COLUMN match_mode TEXT NOT NULL DEFAULT 'substring';",
        )?;
    }

    // Add account_type column if it doesn't exist (for existing databases)
    let has_account_type = conn
        .prepare("SELECT account_type FROM accounts LIMIT 0")
        .is_ok();
    if !has_account_type {
        conn.execute_batch(
            "ALTER TABLE accounts ADD COLUMN account_type TEXT NOT NULL DEFAULT 'unknown';",
        )?;
    }

    // Add manual column if it doesn't exist (for existing databases)
    let has_manual = conn.prepare("SELECT manual FROM accounts LIMIT 0").is_ok();
    if !has_manual {
        conn.execute_batch("ALTER TABLE accounts ADD COLUMN manual INTEGER NOT NULL DEFAULT 0;")?;
    }

    // Add nickname column if it doesn't exist (for existing databases)
    let has_nickname = conn
        .prepare("SELECT nickname FROM accounts LIMIT 0")
        .is_ok();
    if !has_nickname {
        conn.execute_batch("ALTER TABLE accounts ADD COLUMN nickname TEXT;")?;
    }

    // Index creation must happen after ensuring the column exists (older DBs).
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_transactions_category ON transactions(category_id);",
    )?;

    // Create holdings table if it doesn't exist (for existing databases)
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS holdings (
            id TEXT PRIMARY KEY,
            account_id TEXT NOT NULL REFERENCES accounts(id),
            symbol TEXT,
            description TEXT,
            shares TEXT NOT NULL,
            price INTEGER,
            cost_basis INTEGER,
            market_value INTEGER,
            currency TEXT DEFAULT 'USD',
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_holdings_account
            ON holdings(account_id);
        ",
    )?;

    // Migrate holdings table: add price column if missing
    let has_price = conn.prepare("SELECT price FROM holdings LIMIT 0").is_ok();
    if !has_price {
        conn.execute_batch("ALTER TABLE holdings ADD COLUMN price INTEGER;")?;
    }

    // Migrate holdings table: add cost_basis column if missing
    let has_cost_basis = conn
        .prepare("SELECT cost_basis FROM holdings LIMIT 0")
        .is_ok();
    if !has_cost_basis {
        conn.execute_batch("ALTER TABLE holdings ADD COLUMN cost_basis INTEGER;")?;
    }

    // Remove the overly-aggressive UNIQUE(account_id, posted, amount, description) constraint
    // (SQLite requires a table rebuild to remove a table-level UNIQUE constraint).
    let transactions_sql: Option<String> = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'transactions'",
            [],
            |row| row.get(0),
        )
        .optional()?;

    let has_legacy_dedup_constraint = transactions_sql
        .as_deref()
        .is_some_and(|sql| sql.contains("UNIQUE(account_id, posted, amount, description)"));

    if has_legacy_dedup_constraint {
        // Note: this rebuild only runs on very old DBs predating the dedup constraint removal.
        // Older DBs do not have reimburser columns, so we don't need to copy them here —
        // the ADD COLUMN migrations below will add them after this rebuild.
        conn.execute_batch(
            "
            PRAGMA foreign_keys = OFF;
            BEGIN;

            ALTER TABLE transactions RENAME TO transactions_old;

            CREATE TABLE transactions (
                id TEXT PRIMARY KEY,
                account_id TEXT NOT NULL REFERENCES accounts(id),
                posted INTEGER NOT NULL,
                amount INTEGER NOT NULL,
                description TEXT NOT NULL,
                pending INTEGER DEFAULT 0,
                category_id INTEGER REFERENCES categories(id),
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );

            INSERT INTO transactions (id, account_id, posted, amount, description, pending, category_id, created_at, updated_at)
            SELECT id, account_id, posted, amount, description, pending, category_id, created_at, updated_at
            FROM transactions_old;

            DROP TABLE transactions_old;

            CREATE INDEX idx_transactions_account ON transactions(account_id);
            CREATE INDEX idx_transactions_posted ON transactions(posted);
            CREATE INDEX idx_transactions_category ON transactions(category_id);

            COMMIT;
            PRAGMA foreign_keys = ON;
            ",
        )?;
    }

    // Create assets table if it doesn't exist (for existing databases)
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS assets (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            asset_type TEXT NOT NULL,
            description TEXT,
            value INTEGER,
            cost_basis INTEGER,
            currency TEXT DEFAULT 'USD',
            acquired_date INTEGER,
            metadata TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
        ",
    )?;

    // Create rewards_points table if it doesn't exist (for existing databases)
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS rewards_points (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            program TEXT NOT NULL UNIQUE,
            points INTEGER NOT NULL DEFAULT 0,
            note TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
        ",
    )?;

    // Create reimbursers table if it doesn't exist (for existing databases)
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS reimbursers (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL COLLATE NOCASE UNIQUE,
            created_at INTEGER NOT NULL
        );
        ",
    )?;

    // Add reimburser_id column on transactions if missing
    let has_reimburser_id = conn
        .prepare("SELECT reimburser_id FROM transactions LIMIT 0")
        .is_ok();
    if !has_reimburser_id {
        conn.execute_batch(
            "ALTER TABLE transactions ADD COLUMN reimburser_id INTEGER REFERENCES reimbursers(id);",
        )?;
    }

    // Add reimbursed_at column on transactions if missing
    let has_reimbursed_at = conn
        .prepare("SELECT reimbursed_at FROM transactions LIMIT 0")
        .is_ok();
    if !has_reimbursed_at {
        conn.execute_batch("ALTER TABLE transactions ADD COLUMN reimbursed_at INTEGER;")?;
    }

    // Older schemas allowed names that differed only by case. Merge those rows
    // before enforcing the same case-insensitive uniqueness as new databases.
    conn.execute_batch(
        "
        UPDATE transactions
        SET reimburser_id = (
            SELECT MIN(canonical.id)
            FROM reimbursers canonical
            JOIN reimbursers current ON canonical.name = current.name COLLATE NOCASE
            WHERE current.id = transactions.reimburser_id
        )
        WHERE reimburser_id IS NOT NULL;

        DELETE FROM reimbursers
        WHERE id NOT IN (
            SELECT MIN(id) FROM reimbursers GROUP BY name COLLATE NOCASE
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_reimbursers_name_nocase
            ON reimbursers(name COLLATE NOCASE);
        ",
    )?;

    Ok(())
}
