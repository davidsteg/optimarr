-- Device profiles, fix queue, upgrade searches and notifications.

ALTER TABLE media_files ADD COLUMN first_seen_at TEXT;
ALTER TABLE media_files ADD COLUMN upgrade_requested_at TEXT;
UPDATE media_files SET first_seen_at = analyzed_at WHERE first_seen_at IS NULL;
CREATE INDEX IF NOT EXISTS ix_media_first_seen ON media_files(first_seen_at);

-- A client (or group of clients) and what it can direct-play.
CREATE TABLE IF NOT EXISTS device_profiles (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL,
    enabled    INTEGER NOT NULL DEFAULT 1,
    profile    TEXT NOT NULL,               -- JSON DeviceCapabilities
    created_at TEXT NOT NULL
);

-- ffmpeg fixes, run one at a time by the fix worker.
CREATE TABLE IF NOT EXISTS fixes (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    media_file_id  INTEGER REFERENCES media_files(id) ON DELETE SET NULL,
    path           TEXT NOT NULL,
    status         TEXT NOT NULL,           -- queued | running | done | failed | cancelled
    command        TEXT NOT NULL,           -- shell-quoted, for display
    args           TEXT NOT NULL,           -- JSON array passed to ffmpeg
    output_path    TEXT NOT NULL,
    final_path     TEXT,
    progress       REAL NOT NULL DEFAULT 0, -- 0..1
    error          TEXT,
    size_before    INTEGER,
    size_after     INTEGER,
    created_at     TEXT NOT NULL,
    started_at     TEXT,
    finished_at    TEXT
);
CREATE INDEX IF NOT EXISTS ix_fixes_status ON fixes(status);
CREATE INDEX IF NOT EXISTS ix_fixes_media ON fixes(media_file_id);

-- Every notification Optimarr tried to send.
CREATE TABLE IF NOT EXISTS notifications (
    id       INTEGER PRIMARY KEY AUTOINCREMENT,
    event    TEXT NOT NULL,                 -- new_problems | fix_done | fix_failed | upgrades | test
    title    TEXT NOT NULL,
    message  TEXT NOT NULL,
    sent_at  TEXT NOT NULL,
    results  TEXT NOT NULL DEFAULT '[]'     -- JSON [{target, ok, error}]
);
CREATE INDEX IF NOT EXISTS ix_notifications_sent ON notifications(sent_at);
