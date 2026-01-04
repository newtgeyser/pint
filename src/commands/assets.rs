use anyhow::{bail, Context, Result};
use chrono::Utc;

use crate::db::{self, models::Asset};
use crate::util::truncate;

pub const VALID_ASSET_TYPES: &[&str] = &[
    "real_estate",
    "vehicle",
    "collectible",
    "other",
];

pub fn validate_asset_type(t: &str) -> Result<&'static str> {
    let lower = t.to_lowercase().replace(['-', ' '], "_");
    for &valid in VALID_ASSET_TYPES {
        if lower == valid {
            return Ok(valid);
        }
    }
    bail!(
        "Invalid asset type '{}'. Valid types: {}",
        t,
        VALID_ASSET_TYPES.join(", ")
    )
}

pub fn run() -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint init' first.")?;

    let mut stmt = conn.prepare(
        "SELECT id, name, asset_type, description, value, cost_basis, currency, acquired_date, metadata, created_at, updated_at
         FROM assets
         ORDER BY value DESC NULLS LAST",
    )?;

    let assets: Vec<Asset> = stmt
        .query_map([], |row| Asset::from_row(row))?
        .collect::<Result<Vec<_>, _>>()?;

    if assets.is_empty() {
        println!("No assets found. Use 'pint assets add' to add an asset.");
        return Ok(());
    }

    println!(
        "{:<4} {:<12} {:<24} {:>12} {:>12} {:>10}",
        "ID", "TYPE", "NAME", "VALUE", "COST", "GAIN/LOSS"
    );
    println!("{}", "-".repeat(78));

    let mut total_value = 0i64;
    let mut total_cost = 0i64;

    for asset in &assets {
        let value_str = asset
            .value_dollars()
            .map(|v| format!("{:>12.2}", v))
            .unwrap_or_else(|| "         N/A".to_string());

        let cost_str = asset
            .cost_basis_dollars()
            .map(|c| format!("{:>12.2}", c))
            .unwrap_or_else(|| "         N/A".to_string());

        let gain_str = match (asset.cost_basis, asset.value) {
            (Some(cost), Some(value)) if cost > 0 => {
                let gain_pct = ((value - cost) as f64 / cost as f64) * 100.0;
                format!("{:+.1}%", gain_pct)
            }
            _ => "N/A".to_string(),
        };

        println!(
            "{:<4} {:<12} {:<24} {} {} {:>10}",
            asset.id,
            asset.asset_type,
            truncate(&asset.name, 24),
            value_str,
            cost_str,
            gain_str,
        );

        if let Some(v) = asset.value {
            total_value += v;
        }
        if let Some(c) = asset.cost_basis {
            total_cost += c;
        }
    }

    let total_gain_pct = if total_cost > 0 {
        ((total_value - total_cost) as f64 / total_cost as f64) * 100.0
    } else {
        0.0
    };

    println!("{}", "-".repeat(78));
    println!(
        "{:<4} {:<12} {:<24} {:>12.2} {:>12.2} {:>+10.1}%",
        "",
        "",
        "TOTAL",
        total_value as f64 / 100.0,
        total_cost as f64 / 100.0,
        total_gain_pct,
    );

    Ok(())
}

pub fn add(
    name: &str,
    asset_type: &str,
    value: f64,
    cost: Option<f64>,
    description: Option<&str>,
    acquired: Option<i64>,
) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint init' first.")?;
    let asset_type = validate_asset_type(asset_type)?;

    let now = Utc::now().timestamp();
    let value_cents = (value * 100.0).round() as i64;
    let cost_cents = cost.map(|c| (c * 100.0).round() as i64);

    conn.execute(
        "INSERT INTO assets (name, asset_type, description, value, cost_basis, acquired_date, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        rusqlite::params![name, asset_type, description, value_cents, cost_cents, acquired, now],
    )?;

    let id = conn.last_insert_rowid();
    println!("Added asset '{}' (ID: {})", name, id);

    Ok(())
}

pub fn update(
    id: i64,
    name: Option<&str>,
    value: Option<f64>,
    cost: Option<f64>,
    description: Option<&str>,
    asset_type: Option<&str>,
) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint init' first.")?;

    // Verify asset exists
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM assets WHERE id = ?1)",
        [id],
        |row| row.get(0),
    )?;

    if !exists {
        bail!("Asset with ID {} not found", id);
    }

    // Validate asset type if provided
    let validated_type = asset_type.map(validate_asset_type).transpose()?;

    let now = Utc::now().timestamp();

    conn.execute("UPDATE assets SET updated_at = ?1 WHERE id = ?2", rusqlite::params![now, id])?;

    if let Some(n) = name {
        conn.execute("UPDATE assets SET name = ?1 WHERE id = ?2", rusqlite::params![n, id])?;
    }
    if let Some(v) = value {
        let cents = (v * 100.0).round() as i64;
        conn.execute("UPDATE assets SET value = ?1 WHERE id = ?2", rusqlite::params![cents, id])?;
    }
    if let Some(c) = cost {
        let cents = (c * 100.0).round() as i64;
        conn.execute("UPDATE assets SET cost_basis = ?1 WHERE id = ?2", rusqlite::params![cents, id])?;
    }
    if let Some(d) = description {
        conn.execute("UPDATE assets SET description = ?1 WHERE id = ?2", rusqlite::params![d, id])?;
    }
    if let Some(t) = validated_type {
        conn.execute("UPDATE assets SET asset_type = ?1 WHERE id = ?2", rusqlite::params![t, id])?;
    }

    println!("Updated asset ID {}", id);
    Ok(())
}

pub fn remove(id: i64) -> Result<()> {
    let conn = db::open().context("Database not found. Run 'pint init' first.")?;

    let deleted = conn.execute("DELETE FROM assets WHERE id = ?1", [id])?;

    if deleted == 0 {
        bail!("Asset with ID {} not found", id);
    }

    println!("Removed asset ID {}", id);
    Ok(())
}
