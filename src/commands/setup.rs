use anyhow::{bail, Context, Result};
use std::io::{self, Write};

use crate::{db, simplefin::SimpleFin};

const ACCESS_URL_KEY: &str = "simplefin_access_url";

pub fn run() -> Result<()> {
    let conn = db::open().context(
        "Database not found. Run 'pint init' first.",
    )?;

    if db::get_config(&conn, ACCESS_URL_KEY)?.is_some() {
        print!("SimpleFIN is already configured. Overwrite? [y/N] ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

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
