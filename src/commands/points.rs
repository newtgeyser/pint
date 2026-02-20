use anyhow::{bail, Context, Result};
use chrono::Utc;

use crate::db::{self, models::RewardPoints};

fn format_points(n: i64) -> String {
    if n < 0 {
        return format!("-{}", format_points(-n));
    }
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

pub fn run() -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint setup' first.")?;

    let programs = RewardPoints::find_all(&conn)?;

    if programs.is_empty() {
        println!("No reward points programs. Use 'pint points add' or 'pint points set' to add one.");
        return Ok(());
    }

    println!(
        "{:<4} {:<32} {:>12}  {}",
        "ID", "PROGRAM", "POINTS", "NOTE"
    );
    println!("{}", "-".repeat(70));

    let mut total: i64 = 0;

    for p in &programs {
        let note = p.note.as_deref().unwrap_or("");
        println!(
            "{:<4} {:<32} {:>12}  {}",
            p.id,
            p.program,
            format_points(p.points),
            note,
        );
        total += p.points;
    }

    println!("{}", "-".repeat(70));
    println!(
        "{:<4} {:<32} {:>12}",
        "", "TOTAL", format_points(total),
    );

    Ok(())
}

pub fn set(program: &str, points: i64, note: Option<&str>) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint setup' first.")?;
    let now = Utc::now().timestamp();

    conn.execute(
        "INSERT INTO rewards_points (program, points, note, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?4)
         ON CONFLICT(program) DO UPDATE SET
            points = excluded.points,
            note = COALESCE(excluded.note, rewards_points.note),
            updated_at = excluded.updated_at",
        rusqlite::params![program, points, note, now],
    )?;

    println!("Set {} = {} points", program, format_points(points));
    Ok(())
}

pub fn add(program: &str, points: i64, note: Option<&str>) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint setup' first.")?;

    // Check if it already exists
    if RewardPoints::find_by_program(&conn, program)?.is_some() {
        bail!(
            "Program '{}' already exists. Use 'pint points set' to update it.",
            program
        );
    }

    let now = Utc::now().timestamp();
    conn.execute(
        "INSERT INTO rewards_points (program, points, note, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?4)",
        rusqlite::params![program, points, note, now],
    )?;

    println!("Added '{}' with {} points", program, format_points(points));
    Ok(())
}

pub fn remove(program: &str) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint setup' first.")?;

    let deleted = conn.execute(
        "DELETE FROM rewards_points WHERE LOWER(program) = LOWER(?1)",
        [program],
    )?;

    if deleted == 0 {
        bail!("Program '{}' not found", program);
    }

    println!("Removed '{}'", program);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_points() {
        assert_eq!(format_points(0), "0");
        assert_eq!(format_points(100), "100");
        assert_eq!(format_points(1000), "1,000");
        assert_eq!(format_points(85432), "85,432");
        assert_eq!(format_points(1000000), "1,000,000");
    }
}
