/// SQL schema for the asset database

pub const CREATE_SOURCE_FOLDERS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS source_folders (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    label TEXT NOT NULL,
    added_at INTEGER NOT NULL,
    last_scanned_at INTEGER,
    asset_count INTEGER DEFAULT 0,
    status TEXT DEFAULT 'active'
)
"#;

pub const CREATE_FOLDER_SEARCH_CONFIG_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS folder_search_config (
    id INTEGER PRIMARY KEY,
    source_folder_id INTEGER NOT NULL REFERENCES source_folders(id) ON DELETE CASCADE,
    subfolder_prefix TEXT NOT NULL DEFAULT '',
    skip_depth INTEGER NOT NULL DEFAULT 0,
    UNIQUE(source_folder_id, subfolder_prefix)
)
"#;

pub const CREATE_ASSETS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS assets (
    id INTEGER PRIMARY KEY,
    filename TEXT NOT NULL,
    folder_id INTEGER NOT NULL REFERENCES source_folders(id) ON DELETE CASCADE,
    rel_path TEXT NOT NULL,
    zip_file TEXT,
    zip_entry TEXT,
    asset_type TEXT NOT NULL,
    format TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    fs_modified_at INTEGER,

    -- Timestamps
    created_at INTEGER NOT NULL,
    modified_at INTEGER NOT NULL
)
"#;

pub const CREATE_ASSETS_INDEXES: &str = r#"
CREATE INDEX IF NOT EXISTS idx_assets_type ON assets(asset_type);
CREATE INDEX IF NOT EXISTS idx_assets_folder ON assets(folder_id);
CREATE INDEX IF NOT EXISTS idx_assets_modified ON assets(modified_at);
CREATE UNIQUE INDEX IF NOT EXISTS idx_assets_unique ON assets(folder_id, rel_path, COALESCE(zip_file, ''), COALESCE(zip_entry, filename));
"#;

pub const CREATE_IMAGE_METADATA_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS image_metadata (
    asset_id INTEGER PRIMARY KEY REFERENCES assets(id) ON DELETE CASCADE,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    thumbnail_data BLOB,
    processed_at INTEGER NOT NULL
)
"#;

pub const CREATE_AUDIO_METADATA_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS audio_metadata (
    asset_id INTEGER PRIMARY KEY REFERENCES assets(id) ON DELETE CASCADE,
    duration_ms INTEGER NOT NULL,
    sample_rate INTEGER,
    channels INTEGER,
    processed_at INTEGER NOT NULL
)
"#;

pub const CREATE_ASSETS_FTS: &str = r#"
CREATE VIRTUAL TABLE IF NOT EXISTS assets_fts USING fts5(
    filename,
    path_segments,
    tokenize='trigram'
)
"#;

pub const CREATE_FTS_TRIGGERS: &str = r#"
CREATE TRIGGER IF NOT EXISTS assets_ai AFTER INSERT ON assets BEGIN
    INSERT INTO assets_fts(rowid, filename, path_segments)
    VALUES (new.id, new.filename,
        REPLACE(REPLACE(
            new.rel_path || ' ' ||
            COALESCE(new.zip_file || ' ', '') ||
            COALESCE(new.zip_entry, new.filename),
            '/', ' '), '\', ' '));
END;

CREATE TRIGGER IF NOT EXISTS assets_au AFTER UPDATE ON assets BEGIN
    DELETE FROM assets_fts WHERE rowid = old.id;
    INSERT INTO assets_fts(rowid, filename, path_segments)
    VALUES (new.id, new.filename,
        REPLACE(REPLACE(
            new.rel_path || ' ' ||
            COALESCE(new.zip_file || ' ', '') ||
            COALESCE(new.zip_entry, new.filename),
            '/', ' '), '\', ' '));
END;

CREATE TRIGGER IF NOT EXISTS assets_ad AFTER DELETE ON assets BEGIN
    DELETE FROM assets_fts WHERE rowid = old.id;
END;
"#;

pub const CREATE_SCAN_SESSIONS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS scan_sessions (
    id INTEGER PRIMARY KEY,
    source_folder_id INTEGER REFERENCES source_folders(id),
    total_files INTEGER,
    processed_files INTEGER DEFAULT 0,
    status TEXT DEFAULT 'running',
    started_at INTEGER NOT NULL,
    completed_at INTEGER,
    error TEXT
)
"#;

pub const CREATE_PROCESSING_ERRORS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS processing_errors (
    id INTEGER PRIMARY KEY,
    asset_id INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    category TEXT NOT NULL,
    error_message TEXT NOT NULL,
    occurred_at INTEGER NOT NULL,
    retry_count INTEGER DEFAULT 0,
    resolved_at INTEGER
)
"#;

pub const CREATE_PROCESSING_ERRORS_INDEXES: &str = r#"
CREATE INDEX IF NOT EXISTS idx_processing_errors_asset ON processing_errors(asset_id);
CREATE INDEX IF NOT EXISTS idx_processing_errors_category ON processing_errors(category);
CREATE INDEX IF NOT EXISTS idx_processing_errors_unresolved ON processing_errors(category, resolved_at) WHERE resolved_at IS NULL
"#;

/// CLAP audio embeddings for semantic search
pub const CREATE_AUDIO_EMBEDDINGS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS audio_embeddings (
    asset_id INTEGER PRIMARY KEY REFERENCES assets(id) ON DELETE CASCADE,
    embedding BLOB NOT NULL,
    model_version TEXT NOT NULL DEFAULT 'laion/clap-htsat-fused',
    created_at INTEGER NOT NULL
)
"#;

pub const CREATE_AUDIO_EMBEDDINGS_INDEX: &str = r#"
CREATE INDEX IF NOT EXISTS idx_audio_embeddings_model ON audio_embeddings(model_version)
"#;
