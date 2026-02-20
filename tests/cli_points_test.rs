//! Integration tests for reward points functionality.

mod common;

use pint::db::models::RewardPoints;

#[test]
fn test_list_all_reward_points() {
    let conn = common::setup_test_db();

    let programs = RewardPoints::find_all(&conn).unwrap();
    assert_eq!(programs.len(), 4);
}

#[test]
fn test_find_by_program() {
    let conn = common::setup_test_db();

    let sapphire = RewardPoints::find_by_program(&conn, "Chase Sapphire Reserve")
        .unwrap()
        .expect("Should find Chase Sapphire Reserve");

    assert_eq!(sapphire.points, 85432);
    assert_eq!(sapphire.note.as_deref(), Some("Ultimate Rewards"));
}

#[test]
fn test_find_by_program_case_insensitive() {
    let conn = common::setup_test_db();

    let result = RewardPoints::find_by_program(&conn, "chase sapphire reserve")
        .unwrap();
    assert!(result.is_some());
}

#[test]
fn test_find_by_program_not_found() {
    let conn = common::setup_test_db();

    let result = RewardPoints::find_by_program(&conn, "Nonexistent Program")
        .unwrap();
    assert!(result.is_none());
}

#[test]
fn test_set_upsert_new() {
    let conn = common::setup_test_db();

    let initial_count = RewardPoints::find_all(&conn).unwrap().len();

    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT INTO rewards_points (program, points, note, created_at, updated_at)
         VALUES ('Amex Gold', 50000, 'Membership Rewards', ?1, ?1)
         ON CONFLICT(program) DO UPDATE SET
            points = excluded.points,
            note = COALESCE(excluded.note, rewards_points.note),
            updated_at = excluded.updated_at",
        [now],
    ).unwrap();

    let programs = RewardPoints::find_all(&conn).unwrap();
    assert_eq!(programs.len(), initial_count + 1);

    let amex = RewardPoints::find_by_program(&conn, "Amex Gold")
        .unwrap()
        .expect("Should find Amex Gold");
    assert_eq!(amex.points, 50000);
}

#[test]
fn test_set_upsert_existing() {
    let conn = common::setup_test_db();

    let initial_count = RewardPoints::find_all(&conn).unwrap().len();

    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT INTO rewards_points (program, points, note, created_at, updated_at)
         VALUES ('Chase Sapphire Reserve', 90000, NULL, ?1, ?1)
         ON CONFLICT(program) DO UPDATE SET
            points = excluded.points,
            note = COALESCE(excluded.note, rewards_points.note),
            updated_at = excluded.updated_at",
        [now],
    ).unwrap();

    // Count should be unchanged
    let programs = RewardPoints::find_all(&conn).unwrap();
    assert_eq!(programs.len(), initial_count);

    // Points should be updated, note preserved
    let sapphire = RewardPoints::find_by_program(&conn, "Chase Sapphire Reserve")
        .unwrap()
        .unwrap();
    assert_eq!(sapphire.points, 90000);
    assert_eq!(sapphire.note.as_deref(), Some("Ultimate Rewards"));
}

#[test]
fn test_remove_program() {
    let conn = common::setup_test_db();

    let initial_count = RewardPoints::find_all(&conn).unwrap().len();

    conn.execute(
        "DELETE FROM rewards_points WHERE LOWER(program) = LOWER('Chase Amazon Prime')",
        [],
    ).unwrap();

    let programs = RewardPoints::find_all(&conn).unwrap();
    assert_eq!(programs.len(), initial_count - 1);

    let removed = RewardPoints::find_by_program(&conn, "Chase Amazon Prime").unwrap();
    assert!(removed.is_none());
}

#[test]
fn test_total_points() {
    let conn = common::setup_test_db();

    let programs = RewardPoints::find_all(&conn).unwrap();
    let total: i64 = programs.iter().map(|p| p.points).sum();

    // 85432 + 12650 + 4280 + 45000 = 147362
    assert_eq!(total, 147362);
}

#[test]
fn test_programs_sorted_by_name() {
    let conn = common::setup_test_db();

    let programs = RewardPoints::find_all(&conn).unwrap();

    for i in 0..programs.len() - 1 {
        assert!(
            programs[i].program <= programs[i + 1].program,
            "Programs should be sorted alphabetically"
        );
    }
}

#[test]
fn test_reward_points_has_id() {
    let conn = common::setup_test_db();

    let programs = RewardPoints::find_all(&conn).unwrap();
    for p in &programs {
        assert!(p.id > 0);
    }
}
