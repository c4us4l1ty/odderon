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

/// Inject a real database fault from a separate connection, just as a failed
/// migration or damaged persistent row would appear to the authority.
fn exec_database(path: &std::path::Path, sql: &str) {
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
    unsafe {
        let mut db = ptr::null_mut();
        let path = CString::new(path.to_str().unwrap()).unwrap();
        assert_eq!(sqlite3_open(path.as_ptr(), &mut db), 0);
        let sql = CString::new(sql).unwrap();
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
}

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
    let path = std::env::temp_dir().join(format!("odderon-rollback-{}.sqlite", std::process::id()));
    let path_str = path.to_str().unwrap();
    let mut engine = AuthorityEngine::new(SqliteStore::open(path_str).unwrap(), server_key());
    let (fdr, license) = signed_rescue(prediction_a());
    let ticket = commit_for(&mut engine, &fdr, &license);
    // A real database failure on the last write must not leave an Elo update
    // or an orphaned record which a retry could charge for a second time.
    exec_database(&path, "CREATE TRIGGER fail_receipt BEFORE INSERT ON receipts BEGIN SELECT RAISE(ABORT, 'injected failure'); END;");
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

#[test]
fn corrupt_or_mismatched_receipts_cannot_rate_a_retry_twice() {
    let path =
        std::env::temp_dir().join(format!("odderon-bad-receipt-{}.sqlite", std::process::id()));
    let path_str = path.to_str().unwrap();
    let mut engine = AuthorityEngine::new(SqliteStore::open(path_str).unwrap(), server_key());
    let (fdr, license) = signed_rescue(prediction_a());
    let ticket = commit_for(&mut engine, &fdr, &license);
    let first = engine
        .submit(submit_request(&fdr, ticket.clone()), NOW)
        .unwrap();
    let receipt = first.receipt.unwrap();
    let before = engine.store().load_player(&license.public_key()).unwrap();
    let mission = engine.store().load_mission_elo(101).unwrap();
    assert_eq!(before.ranked_count, 1);

    let mut invalid_signature = receipt.clone();
    invalid_signature.server_sig[0] ^= 1;
    let mut other_subject = receipt.payload.clone();
    other_subject.pubkey = [42; 32];
    let wrong_binding = ServerReceipt::issue(other_subject, &server_key());
    assert!(wrong_binding.verify(&engine.server_pk()));
    // Simulate a divergent but genuinely signed same-attempt row, e.g. from
    // inconsistent backup/restore. Both signatures and FDR bindings match,
    // yet the retry and public index must not report different ratings.
    let mut divergent_payload = receipt.payload.clone();
    divergent_payload.elo_after_bits = (receipt.payload.elo_after() + 42.0).to_bits();
    let divergent = ServerReceipt::issue(divergent_payload, &server_key());
    assert!(divergent.verify(&engine.server_pk()));
    assert_ne!(divergent, receipt);
    let public_path = format!("/api/v1/verify/{}", odd_report::hex::encode(&fdr.digest()));
    for bad in [
        divergent.to_bytes(),
        vec![0],
        invalid_signature.to_bytes(),
        wrong_binding.to_bytes(),
    ] {
        let bytes = odd_report::hex::encode(&bad);
        exec_database(
            &path,
            &format!("UPDATE receipts SET receipt=X'{bytes}' WHERE contract=101 AND attempt=1;"),
        );
        let retry = engine.submit(submit_request(&fdr, ticket.clone()), NOW);
        assert!(
            matches!(retry, Err(odd_worker::ServerError::Storage(_))),
            "bad stored receipt must fail closed: {retry:?}"
        );
        let public = odd_worker::http::route(&mut engine, "GET", &public_path, "", NOW);
        assert_eq!(
            public.status, 503,
            "public replay must agree with its idempotency row"
        );
        let after = engine.store().load_player(&license.public_key()).unwrap();
        assert_eq!(after.ranked_count, before.ranked_count);
        assert_eq!(after.elo.to_bits(), before.elo.to_bits());
        assert_eq!(
            engine.store().load_mission_elo(101).unwrap().to_bits(),
            mission.to_bits()
        );
    }
    // The flight index still proves this exact attempt was submitted even if
    // its separate idempotency row was lost. Do not score it again.
    exec_database(
        &path,
        "DELETE FROM receipts WHERE contract=101 AND attempt=1;",
    );
    assert!(matches!(
        engine.submit(submit_request(&fdr, ticket.clone()), NOW),
        Err(odd_worker::ServerError::Storage(_))
    ));
    assert_eq!(
        odd_worker::http::route(&mut engine, "GET", &public_path, "", NOW).status,
        503
    );
    assert_eq!(
        engine
            .store()
            .load_player(&license.public_key())
            .unwrap()
            .ranked_count,
        1
    );
    let bytes = odd_report::hex::encode(&receipt.to_bytes());
    exec_database(
        &path,
        &format!(
            "INSERT INTO receipts(pubkey,contract,attempt,receipt) VALUES(X'{}',101,1,X'{bytes}');",
            odd_report::hex::encode(&license.public_key())
        ),
    );
    let retry = engine.submit(submit_request(&fdr, ticket), NOW).unwrap();
    assert_eq!(retry.receipt, Some(receipt));
    assert_eq!(
        engine
            .store()
            .load_player(&license.public_key())
            .unwrap()
            .ranked_count,
        1
    );
    drop(engine);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{path_str}-wal"));
    let _ = std::fs::remove_file(format!("{path_str}-shm"));
}

#[test]
fn corrupt_commitment_cannot_be_coerced_into_a_valid_ticket_binding() {
    let path = std::env::temp_dir().join(format!(
        "odderon-bad-commitment-{}.sqlite",
        std::process::id()
    ));
    let path_str = path.to_str().unwrap();
    let mut engine = AuthorityEngine::new(SqliteStore::open(path_str).unwrap(), server_key());
    let (fdr, license) = signed_rescue(prediction_a());
    let ticket = commit_for(&mut engine, &fdr, &license);
    // These invalid values previously wrapped, truncated, or coerced to the
    // signed fields while reading the row. They must not authorize a flight.
    let pk = odd_report::hex::encode(&license.public_key());
    let design = odd_report::hex::encode(&ticket.design_hash);
    let prediction = odd_report::hex::encode(&ticket.prediction_hash);
    let timestamp = ticket.server_time_s;
    let mutations = [
        ("mode", "256".to_string(), "0".to_string()),
        ("mode", "'0garbage'".to_string(), "0".to_string()),
        (
            "server_time",
            format!("'{timestamp}garbage'"),
            timestamp.to_string(),
        ),
        (
            "design_hash",
            "CAST(design_hash || X'00' AS BLOB)".to_string(),
            format!("X'{design}'"),
        ),
        (
            "prediction_hash",
            "CAST(prediction_hash || X'00' AS BLOB)".to_string(),
            format!("X'{prediction}'"),
        ),
    ];
    for (column, bad, original) in mutations {
        exec_database(&path, &format!("UPDATE commitments SET {column}={bad} WHERE pubkey=X'{pk}' AND contract=101 AND attempt=1;"));
        let submitted = engine.submit(submit_request(&fdr, ticket.clone()), NOW);
        assert!(
            matches!(submitted, Err(odd_worker::ServerError::Storage(_))),
            "invalid {column}={bad}: {submitted:?}"
        );
        let player = engine.store().load_player(&license.public_key()).unwrap();
        assert_eq!(player.ranked_count, 0);
        assert_eq!(player.elo, 1200.0);
        assert!(engine.store().load_fdr(&fdr.digest()).unwrap().is_none());
        exec_database(&path, &format!("UPDATE commitments SET {column}={original} WHERE pubkey=X'{pk}' AND contract=101 AND attempt=1;"));
    }
    assert!(
        engine
            .submit(submit_request(&fdr, ticket), NOW)
            .unwrap()
            .accepted
    );
    drop(engine);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{path_str}-wal"));
    let _ = std::fs::remove_file(format!("{path_str}-shm"));
}

#[test]
fn corrupt_flight_index_cannot_be_truncated_into_a_valid_public_record() {
    let path =
        std::env::temp_dir().join(format!("odderon-bad-index-{}.sqlite", std::process::id()));
    let path_str = path.to_str().unwrap();
    let mut engine = AuthorityEngine::new(SqliteStore::open(path_str).unwrap(), server_key());
    let (fdr, license) = signed_rescue(prediction_a());
    let ticket = commit_for(&mut engine, &fdr, &license);
    engine
        .submit(submit_request(&fdr, ticket.clone()), NOW)
        .unwrap();
    let public_path = format!("/api/v1/verify/{}", odd_report::hex::encode(&fdr.digest()));
    assert_eq!(
        odd_worker::http::route(&mut engine, "GET", &public_path, "", NOW).status,
        200
    );
    // SQLite's integer accessor accepts text and the old u8 cast wraps 256
    // into Ranked (0), even though the persisted index is invalid. Check all
    // index fields before and after restoring each row.
    let hash = odd_report::hex::encode(&fdr.digest());
    let pubkey = odd_report::hex::encode(&license.public_key());
    let score = fdr.metrics[4].to_string();
    let mutations = [
        ("mode", "256".to_string(), "0".to_string()),
        ("mode", "'0garbage'".to_string(), "0".to_string()),
        ("contract", "4294967397".to_string(), "101".to_string()),
        ("contract", "'101garbage'".to_string(), "101".to_string()),
        ("attempt", "4294967297".to_string(), "1".to_string()),
        ("attempt", "'1garbage'".to_string(), "1".to_string()),
        ("delegation_mask", "256".to_string(), "0".to_string()),
        ("delegation_mask", "'0garbage'".to_string(), "0".to_string()),
        (
            "pubkey",
            "CAST(pubkey || X'00' AS BLOB)".to_string(),
            format!("X'{pubkey}'"),
        ),
        ("score", format!("'{score}garbage'"), score),
    ];
    for (column, bad, original) in mutations {
        exec_database(
            &path,
            &format!("UPDATE fdrs SET {column}={bad} WHERE hash=X'{hash}';"),
        );
        let public = odd_worker::http::route(&mut engine, "GET", &public_path, "", NOW);
        assert_eq!(public.status, 503, "invalid {column}={bad} was accepted");
        let retry = engine.submit(submit_request(&fdr, ticket.clone()), NOW);
        assert!(
            matches!(retry, Err(odd_worker::ServerError::Storage(_))),
            "invalid {column}={bad}: {retry:?}"
        );
        exec_database(
            &path,
            &format!("UPDATE fdrs SET {column}={original} WHERE hash=X'{hash}';"),
        );
    }
    assert_eq!(
        odd_worker::http::route(&mut engine, "GET", &public_path, "", NOW).status,
        200
    );
    assert!(
        engine
            .submit(submit_request(&fdr, ticket), NOW)
            .unwrap()
            .accepted
    );
    drop(engine);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{path_str}-wal"));
    let _ = std::fs::remove_file(format!("{path_str}-shm"));
}

#[test]
fn leaderboards_match_memory_for_order_filter_and_pagination() {
    fn populate<S: Store>(store: &mut S) {
        for i in 0..60u8 {
            let pk = [i; 32];
            store
                .save_player(
                    &pk,
                    &odd_worker::PlayerRow {
                        elo: 1200.0 + f64::from(i),
                        ranked_count: 1,
                        callsign: Some(format!("P{i}")),
                    },
                )
                .unwrap();
            store
                .save_fdr(
                    &[i; 32],
                    &odd_worker::StoredFdr {
                        bytes: vec![],
                        pubkey: pk,
                        contract_id: if i % 2 == 0 { 101 } else { 102 },
                        attempt_num: if i == 59 { 2 } else { 1 },
                        mode: if i == 59 {
                            AttemptMode::Practice
                        } else {
                            AttemptMode::Ranked
                        },
                        score_s: f64::from(i) / 100.0,
                        delegation_mask: 0,
                        receipt: None,
                    },
                )
                .unwrap();
        }
    }
    let mut memory = odd_worker::MemoryStore::new();
    let mut sqlite = SqliteStore::open(":memory:").unwrap();
    populate(&mut memory);
    populate(&mut sqlite);
    for contract in [None, Some(101), Some(102)] {
        for offset in [0, 25, 50] {
            assert_eq!(
                memory.leaderboard(contract, offset, 25).unwrap(),
                sqlite.leaderboard(contract, offset, 25).unwrap()
            );
        }
    }
    let first = sqlite.leaderboard(None, 0, 25).unwrap();
    assert_eq!(first.len(), 25);
    assert_eq!(first[0].pubkey, [58; 32], "practice result must not rank");
    assert_eq!(
        sqlite.leaderboard(Some(101), 0, 25).unwrap()[0].score_s,
        0.58
    );
}
