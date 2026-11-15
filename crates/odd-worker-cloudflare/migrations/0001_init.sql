-- D1 schema for the odd-worker authority (WP 1.9).
-- Mirrors the local SQLite adapter; keys and hashes are hex TEXT here
-- because D1 binds strings most reliably. The storage port hides the
-- representation, so the engine is identical across both adapters.

CREATE TABLE IF NOT EXISTS attempts (
    pubkey TEXT NOT NULL, contract INTEGER NOT NULL, n INTEGER NOT NULL,
    PRIMARY KEY (pubkey, contract)
);
CREATE TABLE IF NOT EXISTS commitments (
    pubkey TEXT NOT NULL, contract INTEGER NOT NULL, attempt INTEGER NOT NULL,
    design_hash TEXT NOT NULL, prediction_hash TEXT NOT NULL,
    mode INTEGER NOT NULL, server_time INTEGER NOT NULL,
    PRIMARY KEY (pubkey, contract, attempt)
);
CREATE TABLE IF NOT EXISTS players (
    pubkey TEXT PRIMARY KEY, elo REAL NOT NULL, ranked INTEGER NOT NULL, callsign TEXT
);
CREATE TABLE IF NOT EXISTS missions (
    contract INTEGER PRIMARY KEY, elo REAL NOT NULL
);
CREATE TABLE IF NOT EXISTS fdrs (
    hash TEXT PRIMARY KEY, pubkey TEXT NOT NULL, contract INTEGER NOT NULL,
    attempt INTEGER NOT NULL, mode INTEGER NOT NULL, score REAL NOT NULL,
    delegation_mask INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS receipts (
    pubkey TEXT NOT NULL, contract INTEGER NOT NULL, attempt INTEGER NOT NULL,
    receipt TEXT NOT NULL, PRIMARY KEY (pubkey, contract, attempt)
);
CREATE TABLE IF NOT EXISTS callsigns (
    name TEXT PRIMARY KEY, pubkey TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS fdrs_by_contract ON fdrs (contract, score DESC);
