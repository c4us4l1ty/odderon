-- All mutations must use a D1 batch whose first statement compares and
-- increments this one global revision. A mismatch violates NOT NULL and
-- rolls the entire batch back; a missing singleton is also made to fail.
CREATE TABLE IF NOT EXISTS authority_revision (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    revision INTEGER NOT NULL CHECK (revision >= 0)
);
INSERT OR IGNORE INTO authority_revision (id, revision) VALUES (1, 0);

-- New R2 objects are immutable and keyed by FDR hash plus payload SHA-256.
-- NULL continues to read the legacy hash-only R2 layout, with full binding
-- checks. Migration cannot repair ratings produced by an older racy adapter.
ALTER TABLE fdrs ADD COLUMN r2_key TEXT;
