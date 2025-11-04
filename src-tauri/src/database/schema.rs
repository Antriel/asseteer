/// SQL schema for the asset database
pub const CREATE_ASSETS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS assets (
    id INTEGER PRIMARY KEY,
    filename TEXT NOT NULL,
    path TEXT NOT NULL,
    zip_entry TEXT,
    asset_type TEXT NOT NULL,
    format TEXT NOT NULL,
    file_size INTEGER NOT NULL,

    -- Image metadata
    width INTEGER,
    height INTEGER,

    -- Audio metadata
    duration_ms INTEGER,
    sample_rate INTEGER,
    channels INTEGER,

    -- Thumbnail (small only for MVP)
    thumbnail_data BLOB,

    -- Timestamps
    created_at INTEGER NOT NULL,
    modified_at INTEGER NOT NULL,

    -- Processing state
    processing_status TEXT DEFAULT 'pending',
    processing_error TEXT
)
"#;

pub const CREATE_ASSETS_INDEXES: &str = r#"
CREATE INDEX IF NOT EXISTS idx_assets_type ON assets(asset_type);
CREATE INDEX IF NOT EXISTS idx_assets_path ON assets(path);
CREATE INDEX IF NOT EXISTS idx_assets_status ON assets(processing_status);
CREATE INDEX IF NOT EXISTS idx_assets_modified ON assets(modified_at);
"#;

pub const CREATE_ASSETS_FTS: &str = r#"
CREATE VIRTUAL TABLE IF NOT EXISTS assets_fts USING fts5(
    filename,
    path_segments,
    content=assets,
    content_rowid=id,
    tokenize='porter unicode61 remove_diacritics 1'
)
"#;

pub const CREATE_FTS_TRIGGERS: &str = r#"
CREATE TRIGGER IF NOT EXISTS assets_ai AFTER INSERT ON assets BEGIN
    INSERT INTO assets_fts(rowid, filename, path_segments)
    VALUES (new.id, new.filename, REPLACE(new.path, '/', ' '));
END;

CREATE TRIGGER IF NOT EXISTS assets_au AFTER UPDATE ON assets BEGIN
    UPDATE assets_fts
    SET filename = new.filename,
        path_segments = REPLACE(new.path, '/', ' ')
    WHERE rowid = new.id;
END;

CREATE TRIGGER IF NOT EXISTS assets_ad AFTER DELETE ON assets BEGIN
    DELETE FROM assets_fts WHERE rowid = old.id;
END;
"#;

pub const CREATE_SCAN_SESSIONS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS scan_sessions (
    id INTEGER PRIMARY KEY,
    root_path TEXT NOT NULL,
    total_files INTEGER,
    processed_files INTEGER DEFAULT 0,
    status TEXT DEFAULT 'running',
    started_at INTEGER NOT NULL,
    completed_at INTEGER,
    error TEXT
)
"#;
