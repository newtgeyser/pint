//! Integration tests for marking transactions as reimbursable.

mod common;

use pint::db::models::{ReimbursableFilter, Reimburser, Transaction, TransactionRow};

fn first_tx_id(conn: &rusqlite::Connection) -> String {
    conn.query_row("SELECT id FROM transactions LIMIT 1", [], |row| row.get(0))
        .unwrap()
}

#[test]
fn test_set_reimburser() {
    let conn = common::setup_test_db();
    let employer_id = Reimburser::insert(&conn, "Employer").unwrap();
    let tx_id = first_tx_id(&conn);

    Transaction::set_reimburser(&conn, &tx_id, employer_id).unwrap();

    let stored: Option<i64> = conn
        .query_row(
            "SELECT reimburser_id FROM transactions WHERE id = ?1",
            [&tx_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stored, Some(employer_id));
}

#[test]
fn test_set_reimburser_resets_paid_status() {
    let conn = common::setup_test_db();
    let employer_id = Reimburser::insert(&conn, "Employer").unwrap();
    let tx_id = first_tx_id(&conn);

    Transaction::set_reimburser(&conn, &tx_id, employer_id).unwrap();
    Transaction::set_reimbursed_at(&conn, &tx_id, Some(123456)).unwrap();
    // Re-set should clear reimbursed_at
    Transaction::set_reimburser(&conn, &tx_id, employer_id).unwrap();

    let reimbursed_at: Option<i64> = conn
        .query_row(
            "SELECT reimbursed_at FROM transactions WHERE id = ?1",
            [&tx_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(reimbursed_at, None);
}

#[test]
fn test_mark_paid_and_clear() {
    let conn = common::setup_test_db();
    let employer_id = Reimburser::insert(&conn, "Employer").unwrap();
    let tx_id = first_tx_id(&conn);

    Transaction::set_reimburser(&conn, &tx_id, employer_id).unwrap();
    Transaction::set_reimbursed_at(&conn, &tx_id, Some(999)).unwrap();

    let stored: Option<i64> = conn
        .query_row(
            "SELECT reimbursed_at FROM transactions WHERE id = ?1",
            [&tx_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stored, Some(999));

    Transaction::clear_reimburser(&conn, &tx_id).unwrap();
    let row: (Option<i64>, Option<i64>) = conn
        .query_row(
            "SELECT reimburser_id, reimbursed_at FROM transactions WHERE id = ?1",
            [&tx_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(row, (None, None));
}

#[test]
fn test_find_reimbursable_filters() {
    let conn = common::setup_test_db();
    let employer_id = Reimburser::insert(&conn, "Employer").unwrap();
    let llc_id = Reimburser::insert(&conn, "My LLC").unwrap();

    // Mark 3 transactions: 1 paid by Employer, 1 pending by Employer, 1 pending by LLC
    let ids: Vec<String> = conn
        .prepare("SELECT id FROM transactions LIMIT 3")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    assert_eq!(ids.len(), 3);

    Transaction::set_reimburser(&conn, &ids[0], employer_id).unwrap();
    Transaction::set_reimbursed_at(&conn, &ids[0], Some(1234)).unwrap();
    Transaction::set_reimburser(&conn, &ids[1], employer_id).unwrap();
    Transaction::set_reimburser(&conn, &ids[2], llc_id).unwrap();

    // All
    let all = TransactionRow::find_reimbursable(&conn, ReimbursableFilter::All, None).unwrap();
    assert_eq!(all.len(), 3);

    // Pending only
    let pending =
        TransactionRow::find_reimbursable(&conn, ReimbursableFilter::Pending, None).unwrap();
    assert_eq!(pending.len(), 2);
    assert!(pending.iter().all(|r| r.reimbursed_at.is_none()));

    // Paid only
    let paid = TransactionRow::find_reimbursable(&conn, ReimbursableFilter::Paid, None).unwrap();
    assert_eq!(paid.len(), 1);
    assert_eq!(paid[0].reimbursed_at, Some(1234));

    // Filter by entity
    let employer_rows =
        TransactionRow::find_reimbursable(&conn, ReimbursableFilter::All, Some("Employer"))
            .unwrap();
    assert_eq!(employer_rows.len(), 2);
    assert!(employer_rows
        .iter()
        .all(|r| r.reimburser.as_deref() == Some("Employer")));

    // Combined filter: pending + entity
    let employer_pending =
        TransactionRow::find_reimbursable(&conn, ReimbursableFilter::Pending, Some("Employer"))
            .unwrap();
    assert_eq!(employer_pending.len(), 1);
}

#[test]
fn test_find_reimbursable_empty_when_none_marked() {
    let conn = common::setup_test_db();
    Reimburser::insert(&conn, "Employer").unwrap();

    let rows = TransactionRow::find_reimbursable(&conn, ReimbursableFilter::All, None).unwrap();
    assert_eq!(rows.len(), 0);
}

#[test]
fn test_transaction_row_includes_reimburser_fields() {
    let conn = common::setup_test_db();
    let employer_id = Reimburser::insert(&conn, "Employer").unwrap();
    let tx_id = first_tx_id(&conn);

    Transaction::set_reimburser(&conn, &tx_id, employer_id).unwrap();

    let rows = TransactionRow::find_all(&conn, None, 200).unwrap();
    let row = rows
        .iter()
        .find(|r| r.id == tx_id)
        .expect("Transaction should appear in find_all");
    assert_eq!(row.reimburser.as_deref(), Some("Employer"));
    assert!(row.reimbursed_at.is_none());
}
