use anyhow::{Context, Result};

use crate::db::{self, models::Reimburser};

pub fn run() -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint setup' first.")?;
    let entities = Reimburser::find_all(&conn)?;

    if entities.is_empty() {
        println!("No reimbursers. Use 'pint reimbursers add <name>' to create one.");
        return Ok(());
    }

    println!("{:<4} {}", "ID", "NAME");
    println!("{}", "-".repeat(40));
    for r in entities {
        println!("{:<4} {}", r.id, r.name);
    }
    Ok(())
}

pub fn add(name: &str) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint setup' first.")?;

    if Reimburser::find_by_name(&conn, name)?.is_some() {
        anyhow::bail!("Reimburser '{}' already exists", name);
    }

    Reimburser::insert(&conn, name)?;
    println!("Added reimburser '{}'", name);
    Ok(())
}

pub fn remove(name: &str) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint setup' first.")?;
    Reimburser::delete_by_name(&conn, name)?;
    println!("Removed reimburser '{}'", name);
    Ok(())
}
