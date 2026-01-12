use anyhow::{bail, Context, Result};
use std::io::{self, Write};

use crate::{config, db, simplefin::SimpleFin};
use super::rules;

const ACCESS_URL_KEY: &str = "simplefin_access_url";

pub fn run() -> Result<()> {
    // Initialize database if needed
    let db_path = config::db_path()?;
    let is_new = !db_path.exists();

    let conn = db::init()?;

    if is_new {
        println!("Initialized database at {}", db_path.display());
    }

    // Import default rules if rules table is empty
    let rule_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM merchant_rules",
        [],
        |row| row.get(0),
    )?;

    if rule_count == 0 {
        let (rules_imported, categories_created) = rules::import_default_rules(&conn)?;
        if rules_imported > 0 || categories_created > 0 {
            println!(
                "Imported {} default rules, created {} categories",
                rules_imported, categories_created
            );
        }
    }

    // Configure SimpleFIN
    if db::get_config(&conn, ACCESS_URL_KEY)?.is_some() {
        print!("SimpleFIN is already configured. Reconfigure? [y/N] ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Setup complete.");
            return Ok(());
        }
    }

    println!();
    println!("Get your setup token from: https://beta-bridge.simplefin.org/");
    println!();
    print!("Paste your SimpleFIN setup token: ");
    io::stdout().flush()?;

    let mut token = String::new();
    io::stdin().read_line(&mut token)?;
    let token = token.trim();

    if token.is_empty() {
        bail!("No token provided");
    }

    println!("Exchanging token...");
    let access_url = SimpleFin::exchange_token(token)?;

    db::set_config(&conn, ACCESS_URL_KEY, &access_url)?;
    println!("SimpleFIN configured successfully!");

    Ok(())
}

pub fn get_access_url(conn: &rusqlite::Connection) -> Result<String> {
    db::get_config(conn, ACCESS_URL_KEY)?
        .context("SimpleFIN not configured. Run 'pint setup' first.")
}
