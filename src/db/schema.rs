use anyhow::Result;
use rusqlite::Connection;

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
            updated_at INTEGER NOT NULL,
            UNIQUE(account_id, posted, amount, description)
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

    Ok(())
}
