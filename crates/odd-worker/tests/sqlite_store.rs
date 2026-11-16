#![cfg(feature = "local")]
//! The local SQLite adapter must implement the same storage port the
//! in-memory store does, persist across a reopen, and carry the authority
//! engine through a full ranked commit→submit with a verifiable receipt.
//! This is the adapter `./scripts/gate.sh` exercises with no cloud creds.

#[path = "common/mod.rs"]
mod common;

use common::*;
use odd_worker::receipt::{ReceiptPayload, ServerReceipt};
use odd_worker::sqlite::SqliteStore;
use odd_worker::{AttemptMode, AuthorityEngine, ServerKey, Store};

#[test]
fn sqlite_attempt_counter_is_atomic_and_monotonic() {
    let mut store = SqliteStore::open(":memory:").unwrap();
    let pk = [3u8; 32];
    assert_eq!(store.increment_attempt(&pk, 101).unwrap(), 1);
    assert_eq!(store.increment_attempt(&pk, 101).unwrap(), 2);
    assert_eq!(store.increment_attempt(&pk, 101).unwrap(), 3);
    // A different contract has its own counter.
    assert_eq!(store.increment_attempt(&pk, 103).unwrap(), 1);
    // A different pilot has its own counter.
    assert_eq!(store.increment_attempt(&[4u8; 32], 101).unwrap(), 1);
}

#[test]
fn sqlite_player_and_mission_round_trip() {
    let mut store = SqliteStore::open(":memory:").unwrap();
    let pk = [5u8; 32];
    let default = store.load_player(&pk).unwrap();
    assert_eq!(default.elo, 1200.0);
    assert_eq!(default.ranked_count, 0);
    assert!(default.callsign.is_none());

    let row = odd_worker::PlayerRow {
        elo: 1234.5,
        ranked_count: 7,
        callsign: Some("ACE".into()),
    };
    store.save_player(&pk, &row).unwrap();
    let loaded = store.load_player(&pk).unwrap();
    assert_eq!(loaded.elo.to_bits(), row.elo.to_bits());
    assert_eq!(loaded.ranked_count, 7);
    assert_eq!(loaded.callsign.as_deref(), Some("ACE"));

    assert_eq!(store.load_mission_elo(103).unwrap(), 1650.0);
    store.save_mission_elo(103, 1640.0).unwrap();
    assert_eq!(
        store.load_mission_elo(103).unwrap().to_bits(),
        1640.0f64.to_bits()
    );
}

#[test]
fn sqlite_receipt_blob_round_trips_and_verifies() {
    let key = ServerKey::from_seed([9u8; 32]);
    let payload = ReceiptPayload::new(
        [1u8; 32],
        [2u8; 32],
        103,
        1,
        AttemptMode::Ranked,
        0.75,
        1200.0,
        1228.0,
        1648.0,
        1_700_000_000,
    );
    let receipt = ServerReceipt::issue(payload, &key);
    let bytes = receipt.to_bytes();
    let parsed = ServerReceipt::from_bytes(&bytes).unwrap();
    assert_eq!(parsed, receipt);
    assert!(parsed.verify(&key.public_key()));
    assert!(ServerReceipt::from_bytes(&bytes[..bytes.len() - 1]).is_none());
}

#[test]
fn sqlite_callsign_is_first_come_first_served() {
    let mut store = SqliteStore::open(":memory:").unwrap();
    let alice = [10u8; 32];
    let bob = [11u8; 32];
    store.register_callsign("NEBULA", &alice).unwrap();
    // Idempotent for the same owner.
    store.register_callsign("NEBULA", &alice).unwrap();
    // Another key cannot take it.
    assert_eq!(
        store.register_callsign("NEBULA", &bob).unwrap_err(),
        odd_worker::ServerError::CallsignTaken
    );
    assert_eq!(
        store.load_player(&alice).unwrap().callsign.as_deref(),
        Some("NEBULA")
    );
}

#[test]
fn the_engine_runs_a_ranked_flight_over_sqlite() {
    let store = SqliteStore::open(":memory:").unwrap();
    let mut engine = AuthorityEngine::new(store, server_key());
    let (fdr, license) = signed_rescue(prediction_a());
    let ticket = commit_for(&mut engine, &fdr, &license);
    assert_eq!(ticket.attempt_num, 1);
    assert_eq!(ticket.mode, AttemptMode::Ranked);

    let response = engine
        .submit(submit_request(&fdr, ticket), NOW)
        .expect("the ranked flight is accepted over SQLite");
    assert!(response.accepted);
    assert_eq!(response.mode, AttemptMode::Ranked);
    assert!(response.elo_after != response.elo_before);
    let receipt = response.receipt.unwrap();
    assert!(receipt.is_certified_ranked(&engine.server_pk()));

    // The rating and the receipt are durably stored.
    let player = engine.store().load_player(&license.public_key()).unwrap();
    assert_eq!(player.elo.to_bits(), response.elo_after.to_bits());
    assert_eq!(player.ranked_count, 1);
    let stored = engine
        .store()
        .load_fdr(&response.fdr_hash)
        .unwrap()
        .unwrap();
    assert_eq!(stored.receipt.unwrap(), receipt);

    // The leaderboard reads the stored flight back.
    let board = engine.store().leaderboard(Some(101), 0, 25).unwrap();
    assert_eq!(board.len(), 1);
    assert_eq!(board[0].fdr_hash, response.fdr_hash);
    assert_eq!(board[0].score_s.to_bits(), response.score_s.to_bits());
}

#[test]
fn sqlite_state_survives_a_reopen() {
    // A unique on-disk path without a wall clock.
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("odderon-test-{}-{n}.sqlite", std::process::id()));
    let path_str = path.to_str().unwrap().to_string();
    let pk = [21u8; 32];

    {
        let mut store = SqliteStore::open(&path_str).unwrap();
        store.increment_attempt(&pk, 101).unwrap();
        store.increment_attempt(&pk, 101).unwrap();
        store
            .save_player(
                &pk,
                &odd_worker::PlayerRow {
                    elo: 1300.0,
                    ranked_count: 2,
                    callsign: Some("DURABLE".into()),
                },
            )
            .unwrap();
    }
    // Reopen: the counter resumes at 3 and the player persists.
    {
        let mut store = SqliteStore::open(&path_str).unwrap();
        assert_eq!(store.increment_attempt(&pk, 101).unwrap(), 3);
        let player = store.load_player(&pk).unwrap();
        assert_eq!(player.elo, 1300.0);
        assert_eq!(player.ranked_count, 2);
        assert_eq!(player.callsign.as_deref(), Some("DURABLE"));
    }
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{path_str}-wal"));
    let _ = std::fs::remove_file(format!("{path_str}-shm"));
}

#[test]
fn sqlite_rolls_back_rating_and_fdr_when_receipt_write_fails() {
    use std::ffi::{c_char, c_int, c_void, CString};
    use std::ptr;
    #[link(name = ":libsqlite3.so.0", kind = "dylib")]
    extern "C" {
        fn sqlite3_open(path: *const c_char, db: *mut *mut c_void) -> c_int;
        fn sqlite3_exec(
            db: *mut c_void,
            sql: *const c_char,
            cb: *mut c_void,
            arg: *mut c_void,
            error: *mut *mut c_char,
        ) -> c_int;
        fn sqlite3_close(db: *mut c_void) -> c_int;
    }
    let path = std::env::temp_dir().join(format!("odderon-rollback-{}.sqlite", std::process::id()));
    let path_str = path.to_str().unwrap();
    let mut engine = AuthorityEngine::new(SqliteStore::open(path_str).unwrap(), server_key());
    let (fdr, license) = signed_rescue(prediction_a());
    let ticket = commit_for(&mut engine, &fdr, &license);
    // A real database failure on the last write must not leave an Elo update
    // or an orphaned record which a retry could charge for a second time.
    unsafe {
        let mut db = ptr::null_mut();
        let path = CString::new(path_str).unwrap();
        assert_eq!(sqlite3_open(path.as_ptr(), &mut db), 0);
        let sql = CString::new("CREATE TRIGGER fail_receipt BEFORE INSERT ON receipts BEGIN SELECT RAISE(ABORT, 'injected failure'); END;").unwrap();
        assert_eq!(
            sqlite3_exec(
                db,
                sql.as_ptr(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut()
            ),
            0
        );
        assert_eq!(sqlite3_close(db), 0);
    }
    assert!(engine.submit(submit_request(&fdr, ticket), NOW).is_err());
    let player = engine.store().load_player(&license.public_key()).unwrap();
    assert_eq!(player.elo, 1200.0);
    assert_eq!(player.ranked_count, 0);
    assert_eq!(engine.store().load_mission_elo(101).unwrap(), 800.0);
    assert!(engine.store().load_fdr(&fdr.digest()).unwrap().is_none());
    drop(engine);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{path_str}-wal"));
    let _ = std::fs::remove_file(format!("{path_str}-shm"));
}
