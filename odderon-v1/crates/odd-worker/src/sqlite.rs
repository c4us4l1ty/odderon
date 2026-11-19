//! Local durable SQLite adapter for the [`Store`] port (WP 1.9).
//!
//! A thin FFI binding to the system `libsqlite3` — no vendored database
//! crate — so the local server persists commitments, ratings and FDRs in
//! the same SQL engine Cloudflare D1 uses. The schema is shared with the
//! D1 adapter. Only parameterized statements are used; no client string is
//! ever concatenated into SQL.
//!
//! [`Store`]: crate::store::Store

use crate::protocol::{AttemptMode, Hash, PubKey, ServerError};
use crate::receipt::ServerReceipt;
use crate::store::{Commitment, LeaderRow, PlayerRow, Store, StoredFdr};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;

#[link(name = ":libsqlite3.so.0", kind = "dylib")]
extern "C" {
    fn sqlite3_open_v2(
        filename: *const c_char,
        pp_db: *mut *mut c_void,
        flags: c_int,
        vfs: *const c_char,
    ) -> c_int;
    fn sqlite3_close(db: *mut c_void) -> c_int;
    fn sqlite3_exec(
        db: *mut c_void,
        sql: *const c_char,
        cb: *mut c_void,
        arg: *mut c_void,
        errmsg: *mut *mut c_char,
    ) -> c_int;
    fn sqlite3_prepare_v2(
        db: *mut c_void,
        sql: *const c_char,
        n_byte: c_int,
        pp_stmt: *mut *mut c_void,
        tail: *mut *const c_char,
    ) -> c_int;
    fn sqlite3_bind_blob(
        stmt: *mut c_void,
        idx: c_int,
        data: *const c_void,
        n: c_int,
        destructor: isize,
    ) -> c_int;
    fn sqlite3_bind_int64(stmt: *mut c_void, idx: c_int, value: i64) -> c_int;
    fn sqlite3_bind_double(stmt: *mut c_void, idx: c_int, value: f64) -> c_int;
    fn sqlite3_bind_null(stmt: *mut c_void, idx: c_int) -> c_int;
    fn sqlite3_bind_text(
        stmt: *mut c_void,
        idx: c_int,
        data: *const c_char,
        n: c_int,
        destructor: isize,
    ) -> c_int;
    fn sqlite3_step(stmt: *mut c_void) -> c_int;
    fn sqlite3_column_blob(stmt: *mut c_void, col: c_int) -> *const c_void;
    fn sqlite3_column_bytes(stmt: *mut c_void, col: c_int) -> c_int;
    fn sqlite3_column_int64(stmt: *mut c_void, col: c_int) -> i64;
    fn sqlite3_column_double(stmt: *mut c_void, col: c_int) -> f64;
    fn sqlite3_column_text(stmt: *mut c_void, col: c_int) -> *const c_char;
    fn sqlite3_finalize(stmt: *mut c_void) -> c_int;
    fn sqlite3_errmsg(db: *mut c_void) -> *const c_char;
}

const OPEN_READWRITE: c_int = 0x0000_0002;
const OPEN_CREATE: c_int = 0x0000_0004;
const OPEN_FULLMUTEX: c_int = 0x0001_0000;
const ROW: c_int = 100;
const DONE: c_int = 101;
/// SQLITE_TRANSIENT: SQLite copies the bound buffer immediately.
const TRANSIENT: isize = -1;

fn storage_err(context: &str) -> ServerError {
    ServerError::Storage(context.to_string())
}

/// The schema, identical to the Cloudflare D1 migration.
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS attempts (
    pubkey BLOB NOT NULL, contract INTEGER NOT NULL, n INTEGER NOT NULL,
    PRIMARY KEY (pubkey, contract)
);
CREATE TABLE IF NOT EXISTS commitments (
    pubkey BLOB NOT NULL, contract INTEGER NOT NULL, attempt INTEGER NOT NULL,
    design_hash BLOB NOT NULL, prediction_hash BLOB NOT NULL,
    mode INTEGER NOT NULL, server_time INTEGER NOT NULL,
    PRIMARY KEY (pubkey, contract, attempt)
);
CREATE TABLE IF NOT EXISTS players (
    pubkey BLOB PRIMARY KEY, elo REAL NOT NULL, ranked INTEGER NOT NULL, callsign TEXT
);
CREATE TABLE IF NOT EXISTS missions (
    contract INTEGER PRIMARY KEY, elo REAL NOT NULL
);
CREATE TABLE IF NOT EXISTS fdrs (
    hash BLOB PRIMARY KEY, payload BLOB NOT NULL, pubkey BLOB NOT NULL,
    contract INTEGER NOT NULL, attempt INTEGER NOT NULL, mode INTEGER NOT NULL,
    score REAL NOT NULL, receipt BLOB, delegation_mask INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS receipts (
    pubkey BLOB NOT NULL, contract INTEGER NOT NULL, attempt INTEGER NOT NULL,
    receipt BLOB NOT NULL, PRIMARY KEY (pubkey, contract, attempt)
);
CREATE TABLE IF NOT EXISTS callsigns (
    name TEXT PRIMARY KEY, pubkey BLOB NOT NULL
);
";

/// A owned SQLite connection implementing the storage port.
pub struct SqliteStore {
    db: *mut c_void,
}

// The connection is opened with SQLITE_OPEN_FULLMUTEX and the adapter gives
// the engine exclusive access per operation, so the raw handle may cross
// threads behind the server's mutex.
unsafe impl Send for SqliteStore {}

impl SqliteStore {
    /// Open (creating if needed) a database at `path`, or `":memory:"`.
    pub fn open(path: &str) -> Result<Self, ServerError> {
        let c_path = CString::new(path).map_err(|_| storage_err("path contains a nul"))?;
        let mut db: *mut c_void = ptr::null_mut();
        let rc = unsafe {
            sqlite3_open_v2(
                c_path.as_ptr(),
                &mut db,
                OPEN_READWRITE | OPEN_CREATE | OPEN_FULLMUTEX,
                ptr::null(),
            )
        };
        if rc != 0 {
            return Err(storage_err("sqlite3_open_v2 failed"));
        }
        let store = Self { db };
        store.exec(SCHEMA)?;
        // WAL gives the local server concurrent reads during a write.
        store.exec("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")?;
        Ok(store)
    }

    fn exec(&self, sql: &str) -> Result<(), ServerError> {
        let c_sql = CString::new(sql).map_err(|_| storage_err("sql contains a nul"))?;
        let rc = unsafe {
            sqlite3_exec(
                self.db,
                c_sql.as_ptr(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            )
        };
        if rc != 0 {
            let msg = unsafe { CStr::from_ptr(sqlite3_errmsg(self.db)) };
            return Err(storage_err(&format!("exec: {}", msg.to_string_lossy())));
        }
        Ok(())
    }

    fn prepare(&self, sql: &str) -> Result<Stmt, ServerError> {
        let c_sql = CString::new(sql).map_err(|_| storage_err("sql contains a nul"))?;
        let mut stmt: *mut c_void = ptr::null_mut();
        let rc =
            unsafe { sqlite3_prepare_v2(self.db, c_sql.as_ptr(), -1, &mut stmt, ptr::null_mut()) };
        if rc != 0 || stmt.is_null() {
            let msg = unsafe { CStr::from_ptr(sqlite3_errmsg(self.db)) };
            return Err(storage_err(&format!("prepare: {}", msg.to_string_lossy())));
        }
        Ok(Stmt { stmt })
    }
}

impl Drop for SqliteStore {
    fn drop(&mut self) {
        unsafe {
            sqlite3_close(self.db);
        }
    }
}

/// A RAII prepared statement.
struct Stmt {
    stmt: *mut c_void,
}

impl Stmt {
    fn blob(&self, idx: c_int, data: &[u8]) -> Result<(), ServerError> {
        let rc = unsafe {
            sqlite3_bind_blob(
                self.stmt,
                idx,
                data.as_ptr() as *const c_void,
                data.len() as c_int,
                TRANSIENT,
            )
        };
        if rc != 0 {
            return Err(storage_err("bind_blob"));
        }
        Ok(())
    }
    fn int(&self, idx: c_int, value: i64) -> Result<(), ServerError> {
        if unsafe { sqlite3_bind_int64(self.stmt, idx, value) } != 0 {
            return Err(storage_err("bind_int64"));
        }
        Ok(())
    }
    fn real(&self, idx: c_int, value: f64) -> Result<(), ServerError> {
        if unsafe { sqlite3_bind_double(self.stmt, idx, value) } != 0 {
            return Err(storage_err("bind_double"));
        }
        Ok(())
    }
    fn null(&self, idx: c_int) -> Result<(), ServerError> {
        if unsafe { sqlite3_bind_null(self.stmt, idx) } != 0 {
            return Err(storage_err("bind_null"));
        }
        Ok(())
    }
    fn text(&self, idx: c_int, value: &str) -> Result<(), ServerError> {
        let rc = unsafe {
            sqlite3_bind_text(
                self.stmt,
                idx,
                value.as_ptr() as *const c_char,
                value.len() as c_int,
                TRANSIENT,
            )
        };
        if rc != 0 {
            return Err(storage_err("bind_text"));
        }
        Ok(())
    }
    /// Step once; true if a row is available.
    fn row(&self) -> Result<bool, ServerError> {
        match unsafe { sqlite3_step(self.stmt) } {
            ROW => Ok(true),
            DONE => Ok(false),
            _ => Err(storage_err("step")),
        }
    }
    fn col_blob(&self, col: c_int) -> Vec<u8> {
        unsafe {
            let ptr = sqlite3_column_blob(self.stmt, col) as *const u8;
            let n = sqlite3_column_bytes(self.stmt, col) as usize;
            if ptr.is_null() || n == 0 {
                Vec::new()
            } else {
                std::slice::from_raw_parts(ptr, n).to_vec()
            }
        }
    }
    fn col_hash(&self, col: c_int) -> Hash {
        let bytes = self.col_blob(col);
        let mut out = [0u8; 32];
        let n = bytes.len().min(32);
        out[..n].copy_from_slice(&bytes[..n]);
        out
    }
    fn col_int(&self, col: c_int) -> i64 {
        unsafe { sqlite3_column_int64(self.stmt, col) }
    }
    fn col_real(&self, col: c_int) -> f64 {
        unsafe { sqlite3_column_double(self.stmt, col) }
    }
    fn col_text(&self, col: c_int) -> Option<String> {
        unsafe {
            let ptr = sqlite3_column_text(self.stmt, col);
            if ptr.is_null() {
                None
            } else {
                Some(CStr::from_ptr(ptr).to_string_lossy().into_owned())
            }
        }
    }
}

impl Drop for Stmt {
    fn drop(&mut self) {
        unsafe {
            sqlite3_finalize(self.stmt);
        }
    }
}

impl Store for SqliteStore {
    fn begin(&mut self) -> Result<(), ServerError> {
        self.exec("BEGIN IMMEDIATE;")
    }
    fn commit(&mut self) -> Result<(), ServerError> {
        self.exec("COMMIT;")
    }
    fn rollback(&mut self) -> Result<(), ServerError> {
        self.exec("ROLLBACK;")
    }

    fn increment_attempt(&mut self, pubkey: &PubKey, contract_id: u32) -> Result<u32, ServerError> {
        // Serialize the bump in SQLite and stop at the protocol's u32 limit.
        // A truncating cast would eventually wrap to 1 and issue another
        // ranked ticket. A malformed row also fails without mutating it.
        let stmt = self.prepare(
            "INSERT INTO attempts (pubkey, contract, n) VALUES (?1, ?2, 1)
             ON CONFLICT(pubkey, contract) DO UPDATE SET n = n + 1
             WHERE typeof(n) = 'integer' AND n BETWEEN 1 AND 4294967294
             RETURNING n;",
        )?;
        stmt.blob(1, pubkey)?;
        stmt.int(2, contract_id as i64)?;
        if !stmt.row()? {
            return Err(storage_err("attempt counter exhausted or invalid"));
        }
        u32::try_from(stmt.col_int(0))
            .ok()
            .filter(|attempt| *attempt != 0)
            .ok_or_else(|| storage_err("attempt counter exhausted or invalid"))
    }

    fn save_commitment(
        &mut self,
        pubkey: &PubKey,
        contract_id: u32,
        attempt_num: u32,
        c: &Commitment,
    ) -> Result<(), ServerError> {
        let stmt = self.prepare(
            "INSERT OR REPLACE INTO commitments
             (pubkey, contract, attempt, design_hash, prediction_hash, mode, server_time)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7);",
        )?;
        stmt.blob(1, pubkey)?;
        stmt.int(2, contract_id as i64)?;
        stmt.int(3, attempt_num as i64)?;
        stmt.blob(4, &c.design_hash)?;
        stmt.blob(5, &c.prediction_hash)?;
        stmt.int(6, c.mode as i64)?;
        stmt.int(7, c.server_time_s)?;
        stmt.row()?;
        Ok(())
    }

    fn load_commitment(
        &self,
        pubkey: &PubKey,
        contract_id: u32,
        attempt_num: u32,
    ) -> Result<Option<Commitment>, ServerError> {
        let stmt = self.prepare(
            "SELECT design_hash, prediction_hash, mode, server_time, typeof(mode), typeof(server_time)
             FROM commitments WHERE pubkey=?1 AND contract=?2 AND attempt=?3;",
        )?;
        stmt.blob(1, pubkey)?;
        stmt.int(2, contract_id as i64)?;
        stmt.int(3, attempt_num as i64)?;
        if !stmt.row()? {
            return Ok(None);
        }
        let invalid = || storage_err("invalid stored commitment");
        if stmt.col_text(4).as_deref() != Some("integer")
            || stmt.col_text(5).as_deref() != Some("integer")
        {
            return Err(invalid());
        }
        Ok(Some(Commitment {
            design_hash: stmt.col_blob(0).try_into().map_err(|_| invalid())?,
            prediction_hash: stmt.col_blob(1).try_into().map_err(|_| invalid())?,
            mode: u8::try_from(stmt.col_int(2))
                .ok()
                .and_then(AttemptMode::from_u8)
                .ok_or_else(invalid)?,
            server_time_s: stmt.col_int(3),
        }))
    }

    fn load_player(&self, pubkey: &PubKey) -> Result<PlayerRow, ServerError> {
        let stmt = self.prepare(
            "SELECT elo, ranked, callsign, typeof(ranked), typeof(elo) FROM players WHERE pubkey=?1;",
        )?;
        stmt.blob(1, pubkey)?;
        if !stmt.row()? {
            return Ok(PlayerRow::default());
        }
        if stmt.col_text(3).as_deref() != Some("integer") {
            return Err(storage_err("invalid ranked counter"));
        }
        let elo = stmt.col_real(0);
        if !matches!(stmt.col_text(4).as_deref(), Some("integer" | "real")) || !elo.is_finite() {
            return Err(storage_err("invalid player rating"));
        }
        Ok(PlayerRow {
            elo,
            ranked_count: u32::try_from(stmt.col_int(1))
                .map_err(|_| storage_err("invalid ranked counter"))?,
            callsign: stmt.col_text(2),
        })
    }

    fn save_player(&mut self, pubkey: &PubKey, row: &PlayerRow) -> Result<(), ServerError> {
        if !row.elo.is_finite() {
            return Err(storage_err("invalid player rating"));
        }
        let stmt = self.prepare(
            "INSERT OR REPLACE INTO players (pubkey, elo, ranked, callsign) VALUES (?1, ?2, ?3, ?4);",
        )?;
        stmt.blob(1, pubkey)?;
        stmt.real(2, row.elo)?;
        stmt.int(3, row.ranked_count as i64)?;
        match &row.callsign {
            Some(name) => stmt.text(4, name)?,
            None => stmt.null(4)?,
        }
        stmt.row()?;
        Ok(())
    }

    fn load_mission_elo(&self, contract_id: u32) -> Result<f64, ServerError> {
        let stmt = self.prepare("SELECT elo, typeof(elo) FROM missions WHERE contract=?1;")?;
        stmt.int(1, contract_id as i64)?;
        if !stmt.row()? {
            return Ok(crate::store::seeded_mission_elo(contract_id));
        }
        let elo = stmt.col_real(0);
        if !matches!(stmt.col_text(1).as_deref(), Some("integer" | "real")) || !elo.is_finite() {
            return Err(storage_err("invalid mission rating"));
        }
        Ok(elo)
    }

    fn save_mission_elo(&mut self, contract_id: u32, elo: f64) -> Result<(), ServerError> {
        if !elo.is_finite() {
            return Err(storage_err("invalid mission rating"));
        }
        let stmt =
            self.prepare("INSERT OR REPLACE INTO missions (contract, elo) VALUES (?1, ?2);")?;
        stmt.int(1, contract_id as i64)?;
        stmt.real(2, elo)?;
        stmt.row()?;
        Ok(())
    }

    fn save_fdr(&mut self, hash: &Hash, fdr: &StoredFdr) -> Result<(), ServerError> {
        let receipt_bytes = fdr
            .receipt
            .as_ref()
            .map(|r| r.to_bytes())
            .unwrap_or_default();
        let stmt = self.prepare(
            "INSERT OR REPLACE INTO fdrs
             (hash, payload, pubkey, contract, attempt, mode, score, receipt, delegation_mask)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9);",
        )?;
        stmt.blob(1, hash)?;
        stmt.blob(2, &fdr.bytes)?;
        stmt.blob(3, &fdr.pubkey)?;
        stmt.int(4, fdr.contract_id as i64)?;
        stmt.int(5, fdr.attempt_num as i64)?;
        stmt.int(6, fdr.mode as i64)?;
        stmt.real(7, fdr.score_s)?;
        stmt.blob(8, &receipt_bytes)?;
        stmt.int(9, fdr.delegation_mask as i64)?;
        stmt.row()?;
        Ok(())
    }

    fn load_fdr(&self, hash: &Hash) -> Result<Option<StoredFdr>, ServerError> {
        let stmt = self.prepare(
            "SELECT payload, pubkey, contract, attempt, mode, score, receipt, delegation_mask,
                    typeof(contract), typeof(attempt), typeof(mode), typeof(score), typeof(delegation_mask)
             FROM fdrs WHERE hash=?1;",
        )?;
        stmt.blob(1, hash)?;
        if !stmt.row()? {
            return Ok(None);
        }
        // SQLite's column accessors coerce text to numbers and Rust's `as`
        // casts truncate. Validate the original storage classes and ranges
        // before comparing these index fields to a signed record.
        if [8, 9, 10, 12]
            .iter()
            .any(|column| stmt.col_text(*column).as_deref() != Some("integer"))
            || !matches!(stmt.col_text(11).as_deref(), Some("integer" | "real"))
        {
            return Err(storage_err("invalid stored flight index"));
        }
        let invalid = || storage_err("invalid stored flight index");
        let pubkey = stmt.col_blob(1).try_into().map_err(|_| invalid())?;
        let contract_id = u32::try_from(stmt.col_int(2)).map_err(|_| invalid())?;
        let attempt_num = u32::try_from(stmt.col_int(3))
            .ok()
            .filter(|attempt| *attempt != 0)
            .ok_or_else(invalid)?;
        let mode = u8::try_from(stmt.col_int(4))
            .ok()
            .and_then(AttemptMode::from_u8)
            .ok_or_else(invalid)?;
        let score_s = stmt.col_real(5);
        if !score_s.is_finite() || !(0.0..=1.0).contains(&score_s) {
            return Err(invalid());
        }
        let delegation_mask = u8::try_from(stmt.col_int(7)).map_err(|_| invalid())?;
        let receipt_bytes = stmt.col_blob(6);
        let receipt = if receipt_bytes.is_empty() {
            None
        } else {
            Some(ServerReceipt::from_bytes(&receipt_bytes).ok_or_else(invalid)?)
        };
        Ok(Some(StoredFdr {
            bytes: stmt.col_blob(0),
            pubkey,
            contract_id,
            attempt_num,
            mode,
            score_s,
            receipt,
            delegation_mask,
        }))
    }

    fn save_receipt(
        &mut self,
        pubkey: &PubKey,
        contract_id: u32,
        attempt_num: u32,
        receipt: &ServerReceipt,
    ) -> Result<(), ServerError> {
        let stmt = self.prepare(
            "INSERT OR REPLACE INTO receipts (pubkey, contract, attempt, receipt)
             VALUES (?1, ?2, ?3, ?4);",
        )?;
        stmt.blob(1, pubkey)?;
        stmt.int(2, contract_id as i64)?;
        stmt.int(3, attempt_num as i64)?;
        stmt.blob(4, &receipt.to_bytes())?;
        stmt.row()?;
        Ok(())
    }

    fn load_receipt(
        &self,
        pubkey: &PubKey,
        contract_id: u32,
        attempt_num: u32,
    ) -> Result<Option<ServerReceipt>, ServerError> {
        let stmt = self.prepare(
            "SELECT receipt FROM receipts WHERE pubkey=?1 AND contract=?2 AND attempt=?3;",
        )?;
        stmt.blob(1, pubkey)?;
        stmt.int(2, contract_id as i64)?;
        stmt.int(3, attempt_num as i64)?;
        if !stmt.row()? {
            return Ok(None);
        }
        // An existing but unreadable receipt is not a missing receipt: if
        // treated as None, the same ranked attempt would be scored again.
        ServerReceipt::from_bytes(&stmt.col_blob(0))
            .map(Some)
            .ok_or_else(|| storage_err("invalid stored receipt"))
    }

    fn register_callsign(&mut self, callsign: &str, pubkey: &PubKey) -> Result<(), ServerError> {
        let check = self.prepare("SELECT pubkey FROM callsigns WHERE name=?1;")?;
        check.text(1, callsign)?;
        if check.row()? {
            let owner = check.col_blob(0);
            if owner.as_slice() != pubkey.as_slice() {
                return Err(ServerError::CallsignTaken);
            }
        }
        drop(check);
        let ins =
            self.prepare("INSERT OR REPLACE INTO callsigns (name, pubkey) VALUES (?1, ?2);")?;
        ins.text(1, callsign)?;
        ins.blob(2, pubkey)?;
        ins.row()?;
        drop(ins);
        let mut row = self.load_player(pubkey)?;
        row.callsign = Some(callsign.to_string());
        self.save_player(pubkey, &row)
    }

    fn leaderboard(
        &self,
        contract_id: Option<u32>,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<LeaderRow>, ServerError> {
        let stmt = match contract_id {
            Some(_) => self.prepare(
                "SELECT f.pubkey, f.score, f.delegation_mask, f.hash, COALESCE(p.elo, 1200), p.callsign
                 FROM fdrs f LEFT JOIN players p ON f.pubkey = p.pubkey
                 WHERE f.mode=0 AND f.contract=?1 ORDER BY f.score DESC, f.hash ASC LIMIT ?2 OFFSET ?3;",
            )?,
            None => self.prepare(
                "SELECT f.pubkey, f.score, f.delegation_mask, f.hash, COALESCE(p.elo, 1200), p.callsign
                 FROM fdrs f LEFT JOIN players p ON f.pubkey = p.pubkey
                 WHERE f.mode=0 ORDER BY COALESCE(p.elo, 1200) DESC, f.hash ASC LIMIT ?1 OFFSET ?2;",
            )?,
        };
        let (limit_pos, offset_pos) = if let Some(id) = contract_id {
            stmt.int(1, id as i64)?;
            (2, 3)
        } else {
            (1, 2)
        };
        stmt.int(limit_pos, limit.min(100) as i64)?;
        stmt.int(offset_pos, offset as i64)?;
        let mut rows = Vec::new();
        while stmt.row()? {
            let pk = stmt.col_blob(0);
            let mut pubkey = [0u8; 32];
            let n = pk.len().min(32);
            pubkey[..n].copy_from_slice(&pk[..n]);
            rows.push(LeaderRow {
                pubkey,
                score_s: stmt.col_real(1),
                delegation_mask: stmt.col_int(2) as u8,
                fdr_hash: stmt.col_hash(3),
                elo: stmt.col_real(4),
                callsign: stmt.col_text(5),
            });
        }
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attempt_counter_fails_closed_at_u32_limit() {
        let mut store = SqliteStore::open(":memory:").unwrap();
        let pk = [3u8; 32];
        store.exec("INSERT INTO attempts(pubkey,contract,n) VALUES (X'0303030303030303030303030303030303030303030303030303030303030303',101,4294967294);").unwrap();
        assert_eq!(store.increment_attempt(&pk, 101).unwrap(), u32::MAX);
        // No wrap to 0, then 1 (which would falsely authorize another ranked
        // attempt); errors must leave the persistent counter unchanged.
        assert!(store.increment_attempt(&pk, 101).is_err());
        assert!(store.increment_attempt(&pk, 101).is_err());
        let stmt = store
            .prepare("SELECT n FROM attempts WHERE contract=101;")
            .unwrap();
        assert!(stmt.row().unwrap());
        assert_eq!(stmt.col_int(0), i64::from(u32::MAX));
        drop(stmt);

        for invalid in ["0", "-1", "4294967296", "1.5"] {
            store
                .exec(&format!(
                    "UPDATE attempts SET n={invalid} WHERE contract=101;"
                ))
                .unwrap();
            assert!(
                store.increment_attempt(&pk, 101).is_err(),
                "invalid stored counter {invalid}"
            );
            // A failed increment must not repair the row by wrapping or
            // silently converting a bad counter into a ranked attempt.
            let stmt = store
                .prepare("SELECT n FROM attempts WHERE contract=101;")
                .unwrap();
            assert!(stmt.row().unwrap());
            assert_eq!(stmt.col_real(0), invalid.parse::<f64>().unwrap());
        }
    }

    #[test]
    fn non_numeric_or_non_finite_ratings_fail_closed() {
        let mut store = SqliteStore::open(":memory:").unwrap();
        let pk = [3u8; 32];
        store.save_player(&pk, &PlayerRow::default()).unwrap();
        store.save_mission_elo(101, 800.0).unwrap();
        for invalid in ["'not a rating'", "1e999", "-1e999"] {
            store
                .exec(&format!("UPDATE players SET elo={invalid};"))
                .unwrap();
            assert!(store.load_player(&pk).is_err(), "player Elo {invalid}");
            store
                .exec(&format!("UPDATE missions SET elo={invalid};"))
                .unwrap();
            assert!(
                store.load_mission_elo(101).is_err(),
                "mission Elo {invalid}"
            );
        }
    }

    #[test]
    fn ranked_counter_rejects_out_of_range_database_values() {
        let store = SqliteStore::open(":memory:").unwrap();
        store.exec("INSERT INTO players(pubkey,elo,ranked,callsign) VALUES (X'0303030303030303030303030303030303030303030303030303030303030303',1200,4294967296,NULL);").unwrap();
        assert!(store.load_player(&[3u8; 32]).is_err());
        store.exec("UPDATE players SET ranked=-1;").unwrap();
        assert!(store.load_player(&[3u8; 32]).is_err());
        store.exec("UPDATE players SET ranked=1.5;").unwrap();
        assert!(store.load_player(&[3u8; 32]).is_err());
    }
}
