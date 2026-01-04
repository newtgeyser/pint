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
            institution TEXT,
            account_type TEXT NOT NULL DEFAULT 'unknown',
            balance INTEGER,
            balance_date INTEGER,
            currency TEXT DEFAULT 'USD',
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

    // Index creation must happen after ensuring the column exists (older DBs).
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_transactions_category ON transactions(category_id);",
    )?;

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

    Ok(())
}
