# Database Schema

## Overview

SQLite database with hybrid storage:
- **Metadata + Embeddings**: SQLite (fast, queryable)
- **Small Thumbnails** (<256KB): SQLite BLOBs
- **Large Thumbnails** (>256KB): Filesystem with directory sharding
- **Full-Text Search**: SQLite FTS5 virtual table

---

## Core Tables

### assets
Main asset metadata table. Stores information about all imported assets.

```sql
CREATE TABLE assets (
    -- Primary key
    id INTEGER PRIMARY KEY,

    -- Basic metadata
    name TEXT NOT NULL,                      -- Filename without extension
    path TEXT NOT NULL,                      -- File path or zip path
    zip_entry TEXT,                          -- Entry name if inside zip
    asset_type TEXT,                         -- 'image' or 'audio'
    format TEXT,                             -- 'png', 'jpg', 'mp3', 'wav', etc.
    file_size INTEGER,                       -- Bytes

    -- Image-specific metadata
    width INTEGER,                           -- Pixels
    height INTEGER,                          -- Pixels
    aspect_ratio TEXT,                       -- '16:9', '1:1', 'portrait', 'landscape'
    is_spritesheet BOOLEAN,                  -- Detected via FFT/CNN
    grid_dimensions TEXT,                    -- '8x8' for sprite sheets

    -- Audio-specific metadata
    duration_ms INTEGER,                     -- Length in milliseconds
    duration_category TEXT,                  -- 'very_short', 'short', 'medium', 'long'
    sample_rate INTEGER,                     -- Hz (e.g., 44100, 48000)
    channels INTEGER,                        -- 1=mono, 2=stereo, etc.

    -- Quality tier tracking
    processing_tier TEXT,                    -- 'fast', 'quality', 'premium'

    -- ML-derived tags (vary by tier)
    auto_tags TEXT,                          -- JSON array: ["pixel art", "character", ...]
    manual_tags TEXT,                        -- JSON array: ["favorite", "rpg", ...]

    -- Premium tier only
    premium_description TEXT,                -- Rich natural language description

    -- Style classification (images)
    style_primary TEXT,                      -- 'pixel_art', 'vector', '3d_render', 'photo', 'hand_drawn'
    style_confidence REAL,                   -- 0.0-1.0

    -- Audio classification (audio)
    sound_category TEXT,                     -- 'impact', 'ambient', 'voice', 'music', 'effect'
    sound_subcategory TEXT,                  -- 'footstep', 'explosion', 'door', 'chirp', etc.
    category_confidence REAL,                -- 0.0-1.0

    -- ML embeddings for similarity search
    embedding BLOB,                          -- 2048-dim float32 vector (serialized)

    -- Perceptual hash for duplicate detection
    perceptual_hash TEXT,                    -- Base64-encoded hash

    -- Spatial layout (UMAP coordinates)
    position_x REAL,                         -- 2D coordinate for canvas layout
    position_y REAL,                         -- 2D coordinate for canvas layout
    cluster_id INTEGER,                      -- HDBSCAN cluster assignment

    -- Thumbnails
    thumbnail_small BLOB,                    -- 128px, <256KB, stored as BLOB
    thumbnail_medium_path TEXT,              -- 512px, stored on filesystem
    thumbnail_large_path TEXT,               -- 2048px, generated on-demand

    -- Timestamps
    created_at INTEGER,                      -- Unix timestamp
    modified_at INTEGER,                     -- Unix timestamp
    last_accessed INTEGER,                   -- Unix timestamp
    cache_version INTEGER DEFAULT 1          -- For invalidation
);

-- Performance indices
CREATE INDEX idx_assets_type ON assets(asset_type);
CREATE INDEX idx_assets_tier ON assets(processing_tier);
CREATE INDEX idx_assets_style ON assets(style_primary);
CREATE INDEX idx_assets_category ON assets(sound_category);
CREATE INDEX idx_assets_duration ON assets(duration_category);
CREATE INDEX idx_assets_hash ON assets(perceptual_hash);
CREATE INDEX idx_assets_modified ON assets(modified_at);
CREATE INDEX idx_assets_cluster ON assets(cluster_id);
```

### Full-Text Search (FTS5)

```sql
-- Virtual table for full-text search
CREATE VIRTUAL TABLE assets_fts USING fts5(
    name,                    -- Asset filename
    path_segments,           -- Space-separated directory names
    auto_tags,               -- ML-generated tags
    manual_tags,             -- User-added tags
    style_desc,              -- Human-readable style description
    sound_desc,              -- Human-readable sound description
    premium_desc,            -- Rich natural language description (premium tier)
    content=assets,          -- Link to main table
    content_rowid=id,
    tokenize='porter unicode61 remove_diacritics 1'
);

-- Keep FTS5 in sync with main table on INSERT
CREATE TRIGGER assets_ai AFTER INSERT ON assets BEGIN
    INSERT INTO assets_fts(rowid, name, path_segments, auto_tags, manual_tags, style_desc, sound_desc, premium_desc)
    VALUES (
        new.id,
        new.name,
        REPLACE(new.path, '/', ' '),
        new.auto_tags,
        new.manual_tags,
        new.style_primary,
        new.sound_category,
        new.premium_description
    );
END;

-- Keep FTS5 in sync with main table on UPDATE
CREATE TRIGGER assets_au AFTER UPDATE ON assets BEGIN
    UPDATE assets_fts
    SET name = new.name,
        path_segments = REPLACE(new.path, '/', ' '),
        auto_tags = new.auto_tags,
        manual_tags = new.manual_tags,
        style_desc = new.style_primary,
        sound_desc = new.sound_category,
        premium_desc = new.premium_description
    WHERE rowid = new.id;
END;

-- Keep FTS5 in sync with main table on DELETE
CREATE TRIGGER assets_ad AFTER DELETE ON assets BEGIN
    DELETE FROM assets_fts WHERE rowid = old.id;
END;
```

### tags
User-defined tag catalog (for manual tagging).

```sql
CREATE TABLE tags (
    id INTEGER PRIMARY KEY,
    name TEXT UNIQUE NOT NULL
);

CREATE INDEX idx_tags_name ON tags(name);
```

### asset_tags
Many-to-many relationship between assets and manual tags.

```sql
CREATE TABLE asset_tags (
    asset_id INTEGER REFERENCES assets(id),
    tag_id INTEGER REFERENCES tags(id),
    PRIMARY KEY (asset_id, tag_id)
);

CREATE INDEX idx_asset_tags_asset ON asset_tags(asset_id);
CREATE INDEX idx_asset_tags_tag ON asset_tags(tag_id);
```

### duplicates
Detected duplicate/similar assets.

```sql
CREATE TABLE duplicates (
    id INTEGER PRIMARY KEY,
    asset_id INTEGER REFERENCES assets(id),
    duplicate_of INTEGER REFERENCES assets(id),
    similarity_score REAL,                   -- 0.0-1.0
    method TEXT,                             -- 'perceptual_hash', 'chromaprint', 'embedding'
    UNIQUE(asset_id, duplicate_of)
);

CREATE INDEX idx_duplicates_asset ON duplicates(asset_id);
CREATE INDEX idx_duplicates_score ON duplicates(similarity_score DESC);
```

---

## Configuration & Optimization

### Pragmas
```sql
-- WAL mode: Better concurrency for multiple readers
PRAGMA journal_mode=WAL;

-- NORMAL: Good safety/performance balance
PRAGMA synchronous=NORMAL;

-- 64MB cache for frequently accessed data
PRAGMA cache_size=-64000;

-- Enable query optimizer
PRAGMA optimize;
```

### Maintenance Queries
```sql
-- Optimize FTS5 index (run after bulk imports)
INSERT INTO assets_fts(assets_fts) VALUES('optimize');

-- Rebuild FTS5 if corrupted
INSERT INTO assets_fts(assets_fts) VALUES('rebuild');

-- Get index statistics
SELECT * FROM assets_fts_data;

-- Vacuum (reclaim space after deletions)
VACUUM;
```

---

## Data Types & Storage

### Embeddings (BLOB)
2048-dimensional float32 vectors stored as binary:
```rust
// Serialize: Vec<f32> → Vec<u8>
let bytes = embedding
    .iter()
    .flat_map(|f| f.to_le_bytes())
    .collect::<Vec<u8>>();

// Deserialize: Vec<u8> → Vec<f32>
let embedding = bytes
    .chunks(4)
    .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
    .collect::<Vec<f32>>();
```

### Tags (JSON)
Stored as JSON arrays for flexible querying:
```json
["pixel art", "character sprite", "knight", "animation"]
```

### Perceptual Hash (TEXT)
Base64-encoded hash for quick duplicate detection:
```text
"iVBORw0KGgoAAAANSUhEUgAAAA=="
```

---

## Storage Estimates (10,000 Assets)

| Component | Per Asset | Total |
|-----------|-----------|-------|
| Metadata (core fields) | 2KB | 20MB |
| Small thumbnail (128px, BLOB) | 20KB | 200MB |
| Medium thumbnail (512px, FS) | 100KB | 1GB |
| Large thumbnail (2048px, FS) | 500KB | 5GB* |
| Embedding (2048-dim) | 8KB | 80MB |
| Perceptual hash | 32 bytes | 320KB |
| **Database Total** | ~30KB | **~300MB** |
| **With All Thumbnails** | - | **~6.3GB** |

*Large thumbnails generated on-demand rather than pre-cached to save space.

### Optimization Strategy
- **Default**: Generate small (128px) + medium (512px) thumbnails
- **On-Demand**: Generate large (2048px) when user zooms in closely
- **Archive**: Store compressed versions of rarely-accessed large thumbnails

---

## Migration Strategy

### Initial Setup
1. Create core `assets` table with indices
2. Create FTS5 virtual table with triggers
3. Create `tags` and `asset_tags` tables
4. Create `duplicates` table

### Incremental Updates
- Auto-tag new imports as they arrive
- Incrementally generate embeddings in background
- Update FTS5 index (triggers handle this automatically)
- Batch cluster recalculation when dataset grows 10%+

### Upgrade Path for Quality Tiers
When user upgrades tier for existing assets:
```sql
UPDATE assets
SET processing_tier = 'quality',
    auto_tags = new_tags,
    embedding = new_embedding,
    style_confidence = new_confidence,
    modified_at = CURRENT_TIMESTAMP
WHERE id IN (select_upgraded_assets);
```

---

## Query Patterns

### Search
```sql
-- Full-text search with filtering and BM25 ranking
SELECT a.*, bm25(fts) as relevance
FROM assets a
INNER JOIN assets_fts fts ON a.id = fts.rowid
WHERE fts MATCH ?
  AND a.asset_type = 'image'
  AND a.style_primary = ?
ORDER BY relevance ASC
LIMIT 50;
```

### Find Similar
```sql
-- Cosine similarity between embeddings
SELECT id, name,
  (SELECT similarity(a.embedding, b.embedding)
   FROM assets b WHERE b.id = ?) as similarity
FROM assets a
WHERE a.asset_type = 'image'
  AND a.id != ?
ORDER BY similarity DESC
LIMIT 20;
```

### Faceted Filtering
```sql
-- Get available filter options with counts
SELECT
  style_primary, COUNT(*) as count
FROM assets
WHERE asset_type = 'image'
GROUP BY style_primary
ORDER BY count DESC;
```

### Find Duplicates
```sql
-- Assets flagged as duplicates of each other
SELECT a.*, d.similarity_score, d.duplicate_of
FROM assets a
INNER JOIN duplicates d ON a.id = d.asset_id
WHERE d.similarity_score > 0.95
ORDER BY a.id, d.similarity_score DESC;
```
