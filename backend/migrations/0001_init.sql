-- Optimarr 2.x schema

CREATE TABLE IF NOT EXISTS settings (
    key        TEXT PRIMARY KEY NOT NULL,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS libraries (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    name                    TEXT NOT NULL,
    path                    TEXT NOT NULL UNIQUE,
    category                TEXT NOT NULL DEFAULT 'other',   -- movies | series | other
    enabled                 INTEGER NOT NULL DEFAULT 1,
    watch                   INTEGER NOT NULL DEFAULT 1,       -- auto-detect new/changed files
    servarr_kind            TEXT,                             -- sonarr | radarr
    servarr_root_folder_id  INTEGER,
    servarr_root_folder_path TEXT,
    created_at              TEXT NOT NULL,
    last_scanned_at         TEXT,
    last_synced_at          TEXT
);

CREATE TABLE IF NOT EXISTS media_files (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id         INTEGER REFERENCES libraries(id) ON DELETE CASCADE,
    path               TEXT NOT NULL UNIQUE,
    file_name          TEXT NOT NULL,
    size               INTEGER NOT NULL DEFAULT 0,
    mtime              INTEGER NOT NULL DEFAULT 0,       -- unix seconds, used for incremental scans
    duration           REAL NOT NULL DEFAULT 0,          -- seconds
    container          TEXT NOT NULL DEFAULT '',
    video_codec        TEXT NOT NULL DEFAULT '',
    video_codec_tag    TEXT NOT NULL DEFAULT '',
    codec_tag_ok       INTEGER NOT NULL DEFAULT 1,
    video_profile      TEXT NOT NULL DEFAULT '',
    bit_depth          INTEGER NOT NULL DEFAULT 8,
    width              INTEGER NOT NULL DEFAULT 0,
    height             INTEGER NOT NULL DEFAULT 0,
    frame_rate         REAL NOT NULL DEFAULT 0,
    video_bitrate      INTEGER NOT NULL DEFAULT 0,       -- bits/s
    overall_bitrate    INTEGER NOT NULL DEFAULT 0,       -- bits/s
    hdr                INTEGER NOT NULL DEFAULT 0,
    hdr_type           TEXT NOT NULL DEFAULT '',
    fast_start         INTEGER NOT NULL DEFAULT 0,
    audio_codecs       TEXT NOT NULL DEFAULT '',         -- comma separated, for filtering
    audio_tracks       TEXT NOT NULL DEFAULT '[]',       -- JSON
    subtitle_formats   TEXT NOT NULL DEFAULT '',         -- comma separated, for filtering
    subtitle_tracks    TEXT NOT NULL DEFAULT '[]',       -- JSON
    compat_score       INTEGER NOT NULL DEFAULT 0,
    compat_verdict     TEXT NOT NULL DEFAULT 'unknown',  -- optimal | good | poor | unknown
    quality_score      INTEGER NOT NULL DEFAULT 0,
    quality_verdict    TEXT NOT NULL DEFAULT 'unknown',  -- excellent | good | fair | poor | unknown
    issues             TEXT NOT NULL DEFAULT '[]',       -- JSON array of Issue
    status             TEXT NOT NULL DEFAULT 'ok',       -- ok | broken | processing | missing
    status_reason      TEXT,
    processing_since   TEXT,
    analyzed_at        TEXT NOT NULL,
    servarr_kind       TEXT,                             -- sonarr | radarr
    sonarr_series_id   INTEGER,
    sonarr_series_title TEXT,
    sonarr_episode_id  INTEGER,
    sonarr_episode_file_id INTEGER,
    season_number      INTEGER,
    episode_number     INTEGER,
    radarr_movie_id    INTEGER,
    radarr_movie_file_id INTEGER,
    radarr_movie_title TEXT,
    year               INTEGER,
    matched_at         TEXT
);

CREATE INDEX IF NOT EXISTS ix_media_library ON media_files(library_id);
CREATE INDEX IF NOT EXISTS ix_media_compat ON media_files(compat_verdict);
CREATE INDEX IF NOT EXISTS ix_media_quality ON media_files(quality_verdict);
CREATE INDEX IF NOT EXISTS ix_media_status ON media_files(status);
CREATE INDEX IF NOT EXISTS ix_media_codec ON media_files(video_codec);
CREATE INDEX IF NOT EXISTS ix_media_analyzed ON media_files(analyzed_at);
CREATE INDEX IF NOT EXISTS ix_media_servarr ON media_files(servarr_kind);

CREATE TABLE IF NOT EXISTS jobs (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    kind          TEXT NOT NULL,           -- scan | match | servarr_sync | playback_sync | rescan
    title         TEXT NOT NULL,
    library_id    INTEGER REFERENCES libraries(id) ON DELETE SET NULL,
    status        TEXT NOT NULL,           -- queued | running | completed | failed | cancelled
    total         INTEGER NOT NULL DEFAULT 0,
    processed     INTEGER NOT NULL DEFAULT 0,
    failed        INTEGER NOT NULL DEFAULT 0,
    skipped       INTEGER NOT NULL DEFAULT 0,
    current_item  TEXT,
    message       TEXT,
    started_at    TEXT NOT NULL,
    finished_at   TEXT
);
CREATE INDEX IF NOT EXISTS ix_jobs_started ON jobs(started_at);
CREATE INDEX IF NOT EXISTS ix_jobs_status ON jobs(status);

CREATE TABLE IF NOT EXISTS job_failures (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id     INTEGER NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    path       TEXT NOT NULL,
    error      TEXT NOT NULL,
    size       INTEGER,
    failed_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS ix_job_failures_job ON job_failures(job_id);

CREATE TABLE IF NOT EXISTS playback_events (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    source_key            TEXT NOT NULL UNIQUE,   -- dedup key from Jellyfin
    jellyfin_item_id      TEXT NOT NULL,
    user_id               TEXT,
    user_name             TEXT,
    item_name             TEXT NOT NULL,
    item_type             TEXT,
    file_path             TEXT NOT NULL DEFAULT '',
    started_at            TEXT NOT NULL,
    stopped_at            TEXT,
    duration_seconds      REAL,
    client_name           TEXT,
    device_name           TEXT,
    device_id             TEXT,
    play_method           TEXT NOT NULL,          -- DirectPlay | DirectStream | Transcode
    transcode_reasons     TEXT,
    transcode_video_codec TEXT,
    transcode_audio_codec TEXT,
    transcode_container   TEXT,
    hw_accel              TEXT,
    synced_at             TEXT NOT NULL,
    media_file_id         INTEGER REFERENCES media_files(id) ON DELETE SET NULL,
    library_id            INTEGER REFERENCES libraries(id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS ix_playback_started ON playback_events(started_at);
CREATE INDEX IF NOT EXISTS ix_playback_method ON playback_events(play_method);
CREATE INDEX IF NOT EXISTS ix_playback_media ON playback_events(media_file_id);
CREATE INDEX IF NOT EXISTS ix_playback_item ON playback_events(jellyfin_item_id);
