//! Integration tests for assets functionality.

mod common;

use pint::db::models::Asset;

#[test]
fn test_list_all_assets() {
    let conn = common::setup_test_db();

    let assets = Asset::find_all(&conn).unwrap();

    // Should have 3 assets from synthetic data
    assert_eq!(assets.len(), 3);
}

#[test]
fn test_assets_sorted_by_type_then_value() {
    let conn = common::setup_test_db();

    let assets = Asset::find_all(&conn).unwrap();

    // Should be sorted by type first, then by value descending within each type
    for i in 0..assets.len() - 1 {
        let type_a = &assets[i].asset_type;
        let type_b = &assets[i + 1].asset_type;
        let val_a = assets[i].value.unwrap_or(0);
        let val_b = assets[i + 1].value.unwrap_or(0);

        if type_a == type_b {
            // Within same type, sorted by value descending
            assert!(val_a >= val_b, "Assets should be sorted by value descending within same type");
        } else {
            // Different types should be in alphabetical order
            assert!(type_a <= type_b, "Assets should be sorted by type alphabetically");
        }
    }
}

#[test]
fn test_real_estate_asset() {
    let conn = common::setup_test_db();

    let assets = Asset::find_all(&conn).unwrap();

    let house = assets.iter()
        .find(|a| a.asset_type == "real_estate")
        .expect("Should have real estate asset");

    assert_eq!(house.name, "Primary Residence");
    assert_eq!(house.value, Some(55000000)); // $550,000
    assert_eq!(house.cost_basis, Some(35000000)); // $350,000
    assert!(house.description.as_ref().unwrap().contains("123 Main Street"));
}

#[test]
fn test_vehicle_asset() {
    let conn = common::setup_test_db();

    let assets = Asset::find_all(&conn).unwrap();

    let car = assets.iter()
        .find(|a| a.asset_type == "vehicle")
        .expect("Should have vehicle asset");

    assert_eq!(car.name, "2020 Toyota Camry");
    assert_eq!(car.value, Some(2200000)); // $22,000
    assert_eq!(car.cost_basis, Some(2800000)); // $28,000 (depreciated)
}

#[test]
fn test_collectible_asset() {
    let conn = common::setup_test_db();

    let assets = Asset::find_all(&conn).unwrap();

    let art = assets.iter()
        .find(|a| a.asset_type == "collectible")
        .expect("Should have collectible asset");

    assert_eq!(art.name, "Art Collection");
    assert_eq!(art.value, Some(1500000)); // $15,000
}

#[test]
fn test_asset_value_in_cents() {
    let conn = common::setup_test_db();

    let assets = Asset::find_all(&conn).unwrap();
    let house = assets.iter().find(|a| a.name == "Primary Residence").unwrap();

    // Value is stored in cents
    assert_eq!(house.value, Some(55000000)); // $550,000 in cents
    assert_eq!(house.value_dollars(), Some(550000.0));
}

#[test]
fn test_asset_cost_basis() {
    let conn = common::setup_test_db();

    let assets = Asset::find_all(&conn).unwrap();
    let house = assets.iter().find(|a| a.name == "Primary Residence").unwrap();

    assert_eq!(house.cost_basis, Some(35000000)); // $350,000 in cents
    assert_eq!(house.cost_basis_dollars(), Some(350000.0));
}

#[test]
fn test_asset_gain_calculation() {
    let conn = common::setup_test_db();

    let assets = Asset::find_all(&conn).unwrap();

    // House: value $550,000, cost $350,000 = $200,000 gain
    let house = assets.iter().find(|a| a.name == "Primary Residence").unwrap();
    let gain = house.value_dollars().unwrap() - house.cost_basis_dollars().unwrap();
    assert!((gain - 200000.0).abs() < 0.01);

    // Car: value $22,000, cost $28,000 = -$6,000 loss (depreciation)
    let car = assets.iter().find(|a| a.asset_type == "vehicle").unwrap();
    let loss = car.value_dollars().unwrap() - car.cost_basis_dollars().unwrap();
    assert!((loss - (-6000.0)).abs() < 0.01);
}

#[test]
fn test_add_asset() {
    let conn = common::setup_test_db();

    let initial_count = Asset::find_all(&conn).unwrap().len();

    // Add a new asset
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT INTO assets (name, asset_type, description, value, cost_basis, currency, created_at, updated_at)
         VALUES ('Rolex Watch', 'collectible', 'Submariner model', 1500000, 1200000, 'USD', ?1, ?1)",
        [now],
    ).unwrap();

    let assets = Asset::find_all(&conn).unwrap();
    assert_eq!(assets.len(), initial_count + 1);

    // Verify the new asset
    let watch = assets.iter().find(|a| a.name == "Rolex Watch");
    assert!(watch.is_some());
    assert_eq!(watch.unwrap().value, Some(1500000));
}

#[test]
fn test_update_asset_value() {
    let conn = common::setup_test_db();

    // Update the car's value (depreciation)
    conn.execute(
        "UPDATE assets SET value = 2000000 WHERE name = '2020 Toyota Camry'",
        [],
    ).unwrap();

    let assets = Asset::find_all(&conn).unwrap();
    let car = assets.iter().find(|a| a.name == "2020 Toyota Camry").unwrap();

    assert_eq!(car.value, Some(2000000)); // $20,000
}

#[test]
fn test_remove_asset() {
    let conn = common::setup_test_db();

    let initial_count = Asset::find_all(&conn).unwrap().len();

    conn.execute("DELETE FROM assets WHERE name = 'Art Collection'", []).unwrap();

    let assets = Asset::find_all(&conn).unwrap();
    assert_eq!(assets.len(), initial_count - 1);

    // Verify it's gone
    let art = assets.iter().find(|a| a.name == "Art Collection");
    assert!(art.is_none());
}

#[test]
fn test_total_assets_value() {
    let conn = common::setup_test_db();

    let assets = Asset::find_all(&conn).unwrap();

    let total: i64 = assets.iter()
        .filter_map(|a| a.value)
        .sum();

    // House: 55,000,000 + Car: 2,200,000 + Art: 1,500,000 = 58,700,000 cents
    assert_eq!(total, 58700000);
}

#[test]
fn test_total_cost_basis() {
    let conn = common::setup_test_db();

    let assets = Asset::find_all(&conn).unwrap();

    let total_cost: i64 = assets.iter()
        .filter_map(|a| a.cost_basis)
        .sum();

    // House: 35,000,000 + Car: 2,800,000 + Art: 800,000 = 38,600,000 cents
    assert_eq!(total_cost, 38600000);
}

#[test]
fn test_assets_currency() {
    let conn = common::setup_test_db();

    let assets = Asset::find_all(&conn).unwrap();

    // All assets should be in USD
    for asset in &assets {
        assert_eq!(asset.currency, "USD");
    }
}

#[test]
fn test_asset_types() {
    let conn = common::setup_test_db();

    let assets = Asset::find_all(&conn).unwrap();

    // Verify all expected asset types are present
    let types: Vec<&str> = assets.iter().map(|a| a.asset_type.as_str()).collect();

    assert!(types.contains(&"real_estate"));
    assert!(types.contains(&"vehicle"));
    assert!(types.contains(&"collectible"));
}

#[test]
fn test_asset_has_id() {
    let conn = common::setup_test_db();

    let assets = Asset::find_all(&conn).unwrap();

    // All assets should have auto-generated IDs
    for asset in &assets {
        assert!(asset.id > 0);
    }
}
