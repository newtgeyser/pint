//! Integration tests for reimburser entity CRUD.

mod common;

use pint::db::models::Reimburser;

#[test]
fn test_list_empty() {
    let conn = common::setup_test_db();
    let entities = Reimburser::find_all(&conn).unwrap();
    assert_eq!(entities.len(), 0);
}

#[test]
fn test_insert_and_list() {
    let conn = common::setup_test_db();

    Reimburser::insert(&conn, "Employer").unwrap();
    Reimburser::insert(&conn, "My LLC").unwrap();

    let entities = Reimburser::find_all(&conn).unwrap();
    assert_eq!(entities.len(), 2);
    // Ordered by name
    assert_eq!(entities[0].name, "Employer");
    assert_eq!(entities[1].name, "My LLC");
}

#[test]
fn test_find_by_name_case_insensitive() {
    let conn = common::setup_test_db();
    Reimburser::insert(&conn, "Employer").unwrap();

    let found = Reimburser::find_by_name(&conn, "employer").unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().name, "Employer");
}

#[test]
fn test_find_by_name_not_found() {
    let conn = common::setup_test_db();
    let found = Reimburser::find_by_name(&conn, "Nonexistent").unwrap();
    assert!(found.is_none());
}

#[test]
fn test_insert_duplicate_fails() {
    let conn = common::setup_test_db();
    Reimburser::insert(&conn, "Employer").unwrap();

    let err = Reimburser::insert(&conn, "Employer");
    assert!(err.is_err(), "Inserting duplicate name should fail");
}

#[test]
fn test_insert_duplicate_with_different_case_fails() {
    let conn = common::setup_test_db();
    Reimburser::insert(&conn, "Employer").unwrap();

    let err = Reimburser::insert(&conn, "employer");
    assert!(err.is_err(), "Names should be unique regardless of case");
}

#[test]
fn test_delete_by_name() {
    let conn = common::setup_test_db();
    Reimburser::insert(&conn, "Employer").unwrap();

    Reimburser::delete_by_name(&conn, "Employer").unwrap();
    let entities = Reimburser::find_all(&conn).unwrap();
    assert_eq!(entities.len(), 0);
}

#[test]
fn test_delete_unknown_fails() {
    let conn = common::setup_test_db();
    let err = Reimburser::delete_by_name(&conn, "Nonexistent");
    assert!(err.is_err());
}

#[test]
fn test_delete_referenced_fails() {
    let conn = common::setup_test_db();
    let id = Reimburser::insert(&conn, "Employer").unwrap();

    // Mark a transaction as reimbursable by Employer
    conn.execute(
        "UPDATE transactions SET reimburser_id = ?1 WHERE id = (SELECT id FROM transactions LIMIT 1)",
        [id],
    )
    .unwrap();

    let err = Reimburser::delete_by_name(&conn, "Employer");
    assert!(err.is_err(), "Deleting referenced reimburser should fail");
    assert!(
        format!("{:?}", err).contains("transaction"),
        "Error should mention transactions"
    );
}

#[test]
fn test_find_all_for_select() {
    let conn = common::setup_test_db();
    Reimburser::insert(&conn, "Employer").unwrap();
    Reimburser::insert(&conn, "My LLC").unwrap();

    let items = Reimburser::find_all_for_select(&conn).unwrap();
    assert_eq!(items.len(), 2);
    // Each is (id, name)
    assert_eq!(items[0].1, "Employer");
    assert_eq!(items[1].1, "My LLC");
}
