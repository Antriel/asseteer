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
    modified_at INTEGER NOT NULL
)
"#;

pub const CREATE_ASSETS_INDEXES: &str = r#"
CREATE INDEX IF NOT EXISTS idx_assets_type ON assets(asset_type);
CREATE INDEX IF NOT EXISTS idx_assets_path ON assets(path);
CREATE INDEX IF NOT EXISTS idx_assets_modified ON assets(modified_at);
"#;

pub const CREATE_PROCESSING_TASKS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS processing_tasks (
    id INTEGER PRIMARY KEY,
    asset_id INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    task_type TEXT NOT NULL,
    status TEXT DEFAULT 'pending',
    priority INTEGER DEFAULT 0,
    retry_count INTEGER DEFAULT 0,
    max_retries INTEGER DEFAULT 3,

    -- Timestamps
    created_at INTEGER NOT NULL,
    started_at INTEGER,
    completed_at INTEGER,

    -- Error tracking
    error_message TEXT,

    -- Progress (item-level)
    progress_current INTEGER DEFAULT 0,
    progress_total INTEGER DEFAULT 1,

    -- Task-specific data (JSON)
    input_params TEXT,
    output_data TEXT
)
"#;

pub const CREATE_PROCESSING_TASKS_INDEXES: &str = r#"
CREATE INDEX IF NOT EXISTS idx_tasks_asset ON processing_tasks(asset_id);
CREATE INDEX IF NOT EXISTS idx_tasks_status ON processing_tasks(status);
CREATE INDEX IF NOT EXISTS idx_tasks_type ON processing_tasks(task_type);
CREATE INDEX IF NOT EXISTS idx_tasks_priority ON processing_tasks(priority DESC, created_at ASC);
"#;

pub const CREATE_ASSETS_FTS: &str = r#"
CREATE VIRTUAL TABLE IF NOT EXISTS assets_fts USING fts5(
    filename,
    path_segments,
    tokenize='porter unicode61 remove_diacritics 1'
)
"#;

pub const CREATE_FTS_TRIGGERS: &str = r#"
CREATE TRIGGER IF NOT EXISTS assets_ai AFTER INSERT ON assets BEGIN
    INSERT INTO assets_fts(rowid, filename, path_segments)
    VALUES (new.id, new.filename, REPLACE(new.path, '/', ' '));
END;

CREATE TRIGGER IF NOT EXISTS assets_au AFTER UPDATE ON assets BEGIN
    DELETE FROM assets_fts WHERE rowid = old.id;
    INSERT INTO assets_fts(rowid, filename, path_segments)
    VALUES (new.id, new.filename, REPLACE(new.path, '/', ' '));
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
