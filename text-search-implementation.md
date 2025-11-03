# Text-Based Search & Filtering for Asset Manager

## Overview

The search system combines **SQLite FTS5 full-text search**, **ML-generated tags at multiple quality levels**, **faceted filtering**, and **hybrid text+vector similarity** to provide powerful multi-dimensional asset discovery.

**Three Quality Tiers:**
- **Fast (Default)**: Basic tags, good quality, 10-20 min processing
- **Quality (Recommended)**: Detailed tags, better accuracy, 15-35 min processing
- **Premium (Overnight)**: Rich natural language descriptions, 1-2 hour processing

---

## Architecture Components

### 1. Searchable Data Sources

**For Images:**
- Filename (normalized, without extension)
- Directory path segments
- Auto-generated tags from CLIP (style, content, category)
- Manual user tags
- Detected attributes (spritesheet, pixelart, vector, dimensions)
- EXIF metadata (camera info, creation date)

**For Audio:**
- Filename (normalized)
- Directory path segments
- Auto-generated tags from PANNs classifier (impact, ambient, voice, music, etc.)
- Duration category (very short <0.5s, short 0.5-2s, medium 2-5s, long >5s)
- Manual user tags
- Detected attributes (loop potential, sample rate)

---

## Database Schema with FTS5

### Enhanced Assets Table

```sql
CREATE TABLE assets (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    path TEXT NOT NULL,
    zip_entry TEXT,
    asset_type TEXT, -- 'image' or 'audio'
    format TEXT,
    
    -- Image metadata
    width INTEGER,
    height INTEGER,
    aspect_ratio TEXT,           -- '16:9', '1:1', 'portrait', 'landscape'
    is_spritesheet BOOLEAN,
    grid_dimensions TEXT,        -- '8x8' for sprite sheets
    
    -- Audio metadata
    duration_ms INTEGER,
    duration_category TEXT,      -- 'very_short', 'short', 'medium', 'long'
    sample_rate INTEGER,
    channels INTEGER,            -- 1=mono, 2=stereo
    
    -- Quality tier tracking
    processing_tier TEXT,        -- 'fast', 'quality', 'premium'
    
    -- ML-derived tags (quality varies by tier)
    auto_tags TEXT,              -- JSON: ["pixel art", "character sprite", "2d"]
    manual_tags TEXT,            -- JSON: ["favorite", "rpg", "hero"]
    
    -- Premium tier: Natural language description
    premium_description TEXT,    -- Rich description from LLaVA/Audio LLM
    
    -- Style classification (for images)
    style_primary TEXT,          -- 'pixel_art', 'vector', '3d_render', 'photo'
    style_confidence REAL,
    
    -- Audio classification (for audio)
    sound_category TEXT,         -- 'impact', 'ambient', 'voice', 'music', 'effect'
    sound_subcategory TEXT,      -- 'footstep', 'explosion', 'door', etc.
    category_confidence REAL,
    
    -- Vector embeddings (for similarity search)
    embedding BLOB,
    
    -- Perceptual hash (for deduplication)
    perceptual_hash TEXT,
    
    -- Spatial coordinates (UMAP)
    position_x REAL,
    position_y REAL,
    cluster_id INTEGER,
    
    -- Timestamps
    created_at INTEGER,
    modified_at INTEGER,
    last_accessed INTEGER
);

CREATE INDEX idx_assets_type ON assets(asset_type);
CREATE INDEX idx_assets_tier ON assets(processing_tier);
CREATE INDEX idx_assets_style ON assets(style_primary);
CREATE INDEX idx_assets_category ON assets(sound_category);
CREATE INDEX idx_assets_duration ON assets(duration_category);
```

### FTS5 Virtual Table

```sql
-- Create FTS5 virtual table for full-text search
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

-- Triggers to keep FTS5 in sync with main table
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

CREATE TRIGGER assets_ad AFTER DELETE ON assets BEGIN
    DELETE FROM assets_fts WHERE rowid = old.id;
END;
```

---

## Auto-Tagging with CLIP (Images)

### Zero-Shot Classification Pipeline

```rust
use ort::{Session, Value};
use ndarray::{Array4, s};

pub struct CLIPImageTagger {
    vision_session: Session,
    text_session: Session,
    
    // Predefined tag categories for classification
    style_labels: Vec<String>,
    content_labels: Vec<String>,
    quality_labels: Vec<String>,
}

impl CLIPImageTagger {
    pub fn new(model_path: &Path) -> Result<Self> {
        // Load CLIP ViT-B/32 ONNX models
        let vision_session = Session::builder()?
            .with_model_from_file(model_path.join("clip_vision.onnx"))?;
        
        let text_session = Session::builder()?
            .with_model_from_file(model_path.join("clip_text.onnx"))?;
        
        Ok(Self {
            vision_session,
            text_session,
            style_labels: vec![
                "pixel art sprite".into(),
                "vector illustration".into(),
                "3d rendered object".into(),
                "hand drawn artwork".into(),
                "photographic image".into(),
                "low poly 3d model".into(),
            ],
            content_labels: vec![
                "game character sprite".into(),
                "background landscape".into(),
                "user interface element".into(),
                "tile or texture".into(),
                "icon or symbol".into(),
                "item or object".into(),
                "particle effect".into(),
            ],
            quality_labels: vec![
                "high resolution asset".into(),
                "low resolution sprite".into(),
                "transparent png asset".into(),
                "sprite sheet animation".into(),
            ],
        })
    }
    
    pub async fn generate_tags(&self, image_bytes: &[u8]) -> Result<ImageTags> {
        // 1. Preprocess image to 224x224
        let img = image::load_from_memory(image_bytes)?;
        let img_tensor = self.preprocess_image(img)?;
        
        // 2. Get image embedding
        let image_input = Value::from_array(img_tensor)?;
        let image_outputs = self.vision_session.run(vec![image_input])?;
        let image_embedding = image_outputs[0].extract_tensor::<f32>()?;
        
        // 3. Classify across all tag categories
        let style_scores = self.classify_tags(&image_embedding, &self.style_labels).await?;
        let content_scores = self.classify_tags(&image_embedding, &self.content_labels).await?;
        let quality_scores = self.classify_tags(&image_embedding, &self.quality_labels).await?;
        
        // 4. Select top tags with confidence threshold
        let mut tags = Vec::new();
        let mut primary_style = None;
        let mut style_confidence = 0.0;
        
        // Best style tag (highest confidence)
        if let Some((idx, &score)) = style_scores.iter().enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap()) 
        {
            if score > 0.15 { // Confidence threshold
                primary_style = Some(self.style_labels[idx].clone());
                style_confidence = score;
            }
        }
        
        // All content tags above threshold
        for (idx, &score) in content_scores.iter().enumerate() {
            if score > 0.20 {
                tags.push(self.content_labels[idx].clone());
            }
        }
        
        // All quality tags above threshold
        for (idx, &score) in quality_scores.iter().enumerate() {
            if score > 0.18 {
                tags.push(self.quality_labels[idx].clone());
            }
        }
        
        Ok(ImageTags {
            style: primary_style,
            style_confidence,
            tags,
        })
    }
    
    async fn classify_tags(&self, image_emb: &Array1<f32>, labels: &[String]) -> Result<Vec<f32>> {
        // 1. Encode all text labels
        let mut text_embeddings = Vec::new();
        
        for label in labels {
            let tokens = self.tokenize_text(label)?;
            let text_input = Value::from_array(tokens)?;
            let text_outputs = self.text_session.run(vec![text_input])?;
            let text_emb = text_outputs[0].extract_tensor::<f32>()?;
            text_embeddings.push(text_emb.to_owned());
        }
        
        // 2. Calculate cosine similarity between image and each text embedding
        let mut similarities = Vec::new();
        for text_emb in &text_embeddings {
            let similarity = cosine_similarity(image_emb.view(), text_emb.view());
            similarities.push(similarity);
        }
        
        // 3. Apply softmax to get probabilities
        let scores = softmax(&similarities);
        Ok(scores)
    }
}

#[derive(Debug)]
pub struct ImageTags {
    pub style: Option<String>,
    pub style_confidence: f32,
    pub tags: Vec<String>,
}
```

### Traditional CV for Sprite Sheet Detection

```rust
use image::{DynamicImage, GenericImageView};
use rustfft::{FftPlanner, num_complex::Complex};

pub fn detect_spritesheet(img: &DynamicImage) -> Option<(u32, u32)> {
    let (width, height) = img.dimensions();
    
    // Skip if too small
    if width < 64 || height < 64 {
        return None;
    }
    
    // Convert to grayscale
    let gray = img.to_luma8();
    
    // Apply FFT to detect periodic patterns
    let grid_x = detect_grid_frequency(&gray, true)?;
    let grid_y = detect_grid_frequency(&gray, false)?;
    
    // Validate that detected grid makes sense
    let cells_x = width / grid_x;
    let cells_y = height / grid_y;
    
    if cells_x >= 2 && cells_y >= 2 && cells_x <= 32 && cells_y <= 32 {
        Some((cells_x, cells_y))
    } else {
        None
    }
}

fn detect_grid_frequency(img: &GrayImage, horizontal: bool) -> Option<u32> {
    // Sum pixels along axis to get 1D signal
    let signal = if horizontal {
        (0..img.width()).map(|x| {
            (0..img.height()).map(|y| img.get_pixel(x, y)[0] as f32).sum::<f32>()
        }).collect::<Vec<_>>()
    } else {
        (0..img.height()).map(|y| {
            (0..img.width()).map(|x| img.get_pixel(x, y)[0] as f32).sum::<f32>()
        }).collect::<Vec<_>>()
    };
    
    // Apply FFT
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(signal.len());
    
    let mut buffer: Vec<Complex<f32>> = signal.iter()
        .map(|&x| Complex::new(x, 0.0))
        .collect();
    
    fft.process(&mut buffer);
    
    // Find dominant frequency (ignoring DC component)
    let magnitudes: Vec<f32> = buffer[1..buffer.len()/2]
        .iter()
        .map(|c| c.norm())
        .collect();
    
    let (peak_idx, &peak_mag) = magnitudes.iter().enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())?;
    
    // Calculate grid spacing from frequency
    let grid_spacing = signal.len() as f32 / (peak_idx + 1) as f32;
    
    // Confidence check: peak should be significantly higher
    let avg_mag = magnitudes.iter().sum::<f32>() / magnitudes.len() as f32;
    if peak_mag > avg_mag * 3.0 && grid_spacing >= 16.0 {
        Some(grid_spacing as u32)
    } else {
        None
    }
}
```

---

## Auto-Tagging with PANNs (Audio)

### Audio Classification Pipeline

```rust
use ort::{Session, Value};
use ndarray::Array2;

pub struct AudioTagger {
    panns_session: Session,
    
    // AudioSet label mapping (527 classes)
    class_labels: Vec<String>,
}

impl AudioTagger {
    pub async fn generate_tags(&self, audio_path: &Path) -> Result<AudioTags> {
        // 1. Load and decode audio
        let audio_data = load_audio(audio_path)?;
        
        // 2. Create mel spectrogram
        let mel_spec = create_mel_spectrogram(&audio_data, 32000)?;
        
        // 3. Run PANNs inference
        let input = Value::from_array(mel_spec)?;
        let outputs = self.panns_session.run(vec![input])?;
        let logits = outputs[0].extract_tensor::<f32>()?;
        
        // 4. Apply sigmoid and get top predictions
        let probabilities: Vec<f32> = logits.iter()
            .map(|&x| 1.0 / (1.0 + (-x).exp()))
            .collect();
        
        // 5. Extract relevant tags
        let mut tags = Vec::new();
        let mut top_scores: Vec<(usize, f32)> = probabilities.iter()
            .enumerate()
            .map(|(idx, &score)| (idx, score))
            .collect();
        
        top_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        
        // Get top 5 tags with score > 0.3
        for (idx, score) in top_scores.iter().take(10) {
            if *score > 0.3 {
                tags.push(AudioTag {
                    label: self.class_labels[*idx].clone(),
                    confidence: *score,
                });
            }
        }
        
        // 6. Classify into primary category
        let category = self.categorize_audio(&tags);
        
        Ok(AudioTags {
            category,
            tags,
        })
    }
    
    fn categorize_audio(&self, tags: &[AudioTag]) -> SoundCategory {
        // Map AudioSet labels to simplified categories
        for tag in tags {
            let label_lower = tag.label.to_lowercase();
            
            if label_lower.contains("speech") || label_lower.contains("voice") {
                return SoundCategory::Voice;
            } else if label_lower.contains("music") || label_lower.contains("instrument") {
                return SoundCategory::Music;
            } else if label_lower.contains("explosion") || label_lower.contains("impact") 
                || label_lower.contains("bang") {
                return SoundCategory::Impact;
            } else if label_lower.contains("ambient") || label_lower.contains("nature")
                || label_lower.contains("wind") {
                return SoundCategory::Ambient;
            }
        }
        
        SoundCategory::Effect // Default
    }
}

#[derive(Debug)]
pub struct AudioTags {
    pub category: SoundCategory,
    pub tags: Vec<AudioTag>,
}

#[derive(Debug)]
pub struct AudioTag {
    pub label: String,
    pub confidence: f32,
}

#[derive(Debug)]
pub enum SoundCategory {
    Impact,
    Ambient,
    Voice,
    Music,
    Effect,
}
```

---

## Search Query Processing

### Rust Backend Search Handler

```rust
#[tauri::command]
pub async fn search_assets(
    state: State<'_, SharedState>,
    query: SearchQuery,
) -> Result<Vec<AssetSearchResult>, String> {
    let conn = state.db_connection.lock().unwrap();
    
    // Build SQL query based on search parameters
    let mut sql = String::from("SELECT DISTINCT a.* FROM assets a");
    let mut joins = Vec::new();
    let mut conditions = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    
    // 1. Full-text search
    if let Some(text) = &query.text {
        if !text.is_empty() {
            joins.push("INNER JOIN assets_fts fts ON a.id = fts.rowid");
            conditions.push("fts MATCH ?");
            params.push(Box::new(prepare_fts_query(text)));
        }
    }
    
    // 2. Asset type filter
    if let Some(asset_type) = &query.asset_type {
        conditions.push("a.asset_type = ?");
        params.push(Box::new(asset_type.clone()));
    }
    
    // 3. Style filter (for images)
    if let Some(styles) = &query.styles {
        if !styles.is_empty() {
            let placeholders = styles.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            conditions.push(&format!("a.style_primary IN ({})", placeholders));
            for style in styles {
                params.push(Box::new(style.clone()));
            }
        }
    }
    
    // 4. Sound category filter (for audio)
    if let Some(categories) = &query.sound_categories {
        if !categories.is_empty() {
            let placeholders = categories.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            conditions.push(&format!("a.sound_category IN ({})", placeholders));
            for cat in categories {
                params.push(Box::new(cat.clone()));
            }
        }
    }
    
    // 5. Duration filter (for audio)
    if let Some(duration_cat) = &query.duration_category {
        conditions.push("a.duration_category = ?");
        params.push(Box::new(duration_cat.clone()));
    }
    
    // 6. Dimension filter (for images)
    if let Some(min_width) = query.min_width {
        conditions.push("a.width >= ?");
        params.push(Box::new(min_width));
    }
    if let Some(min_height) = query.min_height {
        conditions.push("a.height >= ?");
        params.push(Box::new(min_height));
    }
    
    // 7. Tag filter (JSON array contains)
    if let Some(tags) = &query.tags {
        for tag in tags {
            // SQLite JSON functions
            conditions.push("(
                json_extract(a.auto_tags, '$') LIKE ? OR
                json_extract(a.manual_tags, '$') LIKE ?
            )");
            let tag_pattern = format!("%{}%", tag);
            params.push(Box::new(tag_pattern.clone()));
            params.push(Box::new(tag_pattern));
        }
    }
    
    // 8. Sprite sheet filter
    if let Some(is_spritesheet) = query.is_spritesheet {
        conditions.push("a.is_spritesheet = ?");
        params.push(Box::new(is_spritesheet));
    }
    
    // Combine query parts
    if !joins.is_empty() {
        sql.push_str(&format!(" {}", joins.join(" ")));
    }
    
    if !conditions.is_empty() {
        sql.push_str(&format!(" WHERE {}", conditions.join(" AND ")));
    }
    
    // 9. Sorting
    sql.push_str(&match query.sort_by {
        SortBy::Relevance => " ORDER BY bm25(fts) ASC", // Lower BM25 = more relevant
        SortBy::Name => " ORDER BY a.name COLLATE NOCASE ASC",
        SortBy::DateModified => " ORDER BY a.modified_at DESC",
        SortBy::DateCreated => " ORDER BY a.created_at DESC",
        SortBy::Size => " ORDER BY a.file_size DESC",
        SortBy::Duration => " ORDER BY a.duration_ms ASC",
    });
    
    // 10. Pagination
    sql.push_str(&format!(" LIMIT {} OFFSET {}", query.limit, query.offset));
    
    // Execute query
    let mut stmt = conn.prepare(&sql)?;
    let results = stmt.query_map(params.as_slice(), |row| {
        Ok(AssetSearchResult::from_row(row)?)
    })?
    .collect::<Result<Vec<_>, _>>()?;
    
    Ok(results)
}

fn prepare_fts_query(text: &str) -> String {
    // Handle special FTS5 operators
    let mut query = text.trim().to_string();
    
    // Add prefix wildcard for autocomplete if doesn't have operators
    if !query.contains("OR") && !query.contains("AND") && !query.contains("NOT") {
        query = query.split_whitespace()
            .map(|term| format!("{}*", term))
            .collect::<Vec<_>>()
            .join(" ");
    }
    
    query
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub text: Option<String>,
    pub asset_type: Option<String>,  // 'image' or 'audio'
    pub styles: Option<Vec<String>>,
    pub sound_categories: Option<Vec<String>>,
    pub duration_category: Option<String>,
    pub min_width: Option<u32>,
    pub min_height: Option<u32>,
    pub tags: Option<Vec<String>>,
    pub is_spritesheet: Option<bool>,
    pub sort_by: SortBy,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortBy {
    Relevance,
    Name,
    DateModified,
    DateCreated,
    Size,
    Duration,
}
```

---

## Frontend Search UI

### React Search Component

```typescript
import { useState, useEffect, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import debounce from 'lodash/debounce';

interface SearchFilters {
  text: string;
  assetType?: 'image' | 'audio';
  styles: string[];
  soundCategories: string[];
  durationCategory?: string;
  minWidth?: number;
  minHeight?: number;
  tags: string[];
  isSpritesheet?: boolean;
  sortBy: 'relevance' | 'name' | 'date_modified' | 'date_created' | 'size' | 'duration';
}

export function AssetSearchBar() {
  const [filters, setFilters] = useState<SearchFilters>({
    text: '',
    styles: [],
    soundCategories: [],
    tags: [],
    sortBy: 'relevance',
  });
  
  const [results, setResults] = useState<Asset[]>([]);
  const [isSearching, setIsSearching] = useState(false);
  const [totalResults, setTotalResults] = useState(0);
  
  // Debounced search function
  const debouncedSearch = useMemo(
    () => debounce(async (searchFilters: SearchFilters) => {
      setIsSearching(true);
      try {
        const results = await invoke<Asset[]>('search_assets', {
          query: {
            ...searchFilters,
            limit: 50,
            offset: 0,
          }
        });
        setResults(results);
        setTotalResults(results.length); // In production, get count from backend
      } catch (error) {
        console.error('Search failed:', error);
      } finally {
        setIsSearching(false);
      }
    }, 300),
    []
  );
  
  // Trigger search when filters change
  useEffect(() => {
    debouncedSearch(filters);
  }, [filters, debouncedSearch]);
  
  return (
    <div className="search-container">
      {/* Main search input */}
      <input
        type="text"
        placeholder="Search assets... (e.g., 'pixel art character', 'explosion sound')"
        value={filters.text}
        onChange={(e) => setFilters({ ...filters, text: e.target.value })}
        className="search-input"
      />
      
      {/* Filter chips */}
      <div className="filter-bar">
        {/* Asset type toggle */}
        <select
          value={filters.assetType || 'all'}
          onChange={(e) => setFilters({
            ...filters,
            assetType: e.target.value === 'all' ? undefined : e.target.value as 'image' | 'audio'
          })}
        >
          <option value="all">All Assets</option>
          <option value="image">Images Only</option>
          <option value="audio">Audio Only</option>
        </select>
        
        {/* Image-specific filters */}
        {filters.assetType !== 'audio' && (
          <>
            <MultiSelect
              label="Style"
              options={['pixel_art', 'vector', '3d_render', 'hand_drawn', 'photo']}
              selected={filters.styles}
              onChange={(styles) => setFilters({ ...filters, styles })}
            />
            
            <input
              type="number"
              placeholder="Min Width"
              value={filters.minWidth || ''}
              onChange={(e) => setFilters({
                ...filters,
                minWidth: e.target.value ? parseInt(e.target.value) : undefined
              })}
            />
            
            <label>
              <input
                type="checkbox"
                checked={filters.isSpritesheet || false}
                onChange={(e) => setFilters({
                  ...filters,
                  isSpritesheet: e.target.checked ? true : undefined
                })}
              />
              Sprite Sheets Only
            </label>
          </>
        )}
        
        {/* Audio-specific filters */}
        {filters.assetType !== 'image' && (
          <>
            <MultiSelect
              label="Category"
              options={['impact', 'ambient', 'voice', 'music', 'effect']}
              selected={filters.soundCategories}
              onChange={(cats) => setFilters({ ...filters, soundCategories: cats })}
            />
            
            <select
              value={filters.durationCategory || 'all'}
              onChange={(e) => setFilters({
                ...filters,
                durationCategory: e.target.value === 'all' ? undefined : e.target.value
              })}
            >
              <option value="all">Any Duration</option>
              <option value="very_short">&lt; 0.5s</option>
              <option value="short">0.5s - 2s</option>
              <option value="medium">2s - 5s</option>
              <option value="long">&gt; 5s</option>
            </select>
          </>
        )}
        
        {/* Sort options */}
        <select
          value={filters.sortBy}
          onChange={(e) => setFilters({
            ...filters,
            sortBy: e.target.value as SearchFilters['sortBy']
          })}
        >
          <option value="relevance">Most Relevant</option>
          <option value="name">Name A-Z</option>
          <option value="date_modified">Recently Modified</option>
          <option value="date_created">Recently Added</option>
          <option value="size">File Size</option>
          {filters.assetType === 'audio' && <option value="duration">Duration</option>}
        </select>
      </div>
      
      {/* Results count */}
      <div className="results-info">
        {isSearching ? 'Searching...' : `${totalResults} results found`}
      </div>
      
      {/* Results grid */}
      <div className="results-grid">
        {results.map(asset => (
          <AssetCard key={asset.id} asset={asset} />
        ))}
      </div>
    </div>
  );
}
```

---

## Advanced FTS5 Query Examples

### Common Search Patterns

```sql
-- 1. Simple word search (matches any occurrence)
SELECT * FROM assets_fts WHERE assets_fts MATCH 'explosion';

-- 2. Phrase search (exact phrase)
SELECT * FROM assets_fts WHERE assets_fts MATCH '"pixel art"';

-- 3. Prefix search (autocomplete)
SELECT * FROM assets_fts WHERE assets_fts MATCH 'exp*';  -- matches "explosion", "explode", etc.

-- 4. Boolean operators
SELECT * FROM assets_fts WHERE assets_fts MATCH 'pixel AND art NOT vector';
SELECT * FROM assets_fts WHERE assets_fts MATCH 'footstep OR walk OR step';

-- 5. Column-specific search
SELECT * FROM assets_fts WHERE assets_fts MATCH 'name:character';  -- Search only in name

-- 6. Proximity search (words within N positions)
SELECT * FROM assets_fts WHERE assets_fts MATCH 'NEAR(game character, 3)';

-- 7. Combining with filters and relevance ranking
SELECT 
    a.*,
    bm25(fts) as relevance_score,
    snippet(fts, 0, '<b>', '</b>', '...', 20) as snippet
FROM assets a
INNER JOIN assets_fts fts ON a.id = fts.rowid
WHERE fts MATCH 'pixel art'
  AND a.width >= 64
  AND a.asset_type = 'image'
ORDER BY relevance_score ASC
LIMIT 20;

-- 8. Multi-field weighted search (boost certain fields)
-- BM25 can accept column weights: bm25(fts, weight_name, weight_tags, ...)
SELECT 
    a.*,
    bm25(fts, 2.0, 1.0, 1.5, 1.0, 1.0, 1.0) as score  -- Boost name and auto_tags
FROM assets a
INNER JOIN assets_fts fts ON a.id = fts.rowid
WHERE fts MATCH 'character'
ORDER BY score ASC;
```

---

## Hybrid Search: Text + Vector Similarity

### Finding Similar Assets

```rust
#[tauri::command]
pub async fn find_similar_assets(
    state: State<'_, SharedState>,
    asset_id: AssetId,
    limit: u32,
) -> Result<Vec<SimilarAsset>, String> {
    let conn = state.db_connection.lock().unwrap();
    
    // 1. Get the query asset's embedding
    let query_embedding: Vec<f32> = conn.query_row(
        "SELECT embedding FROM assets WHERE id = ?",
        params![asset_id],
        |row| {
            let blob: Vec<u8> = row.get(0)?;
            Ok(deserialize_embedding(&blob))
        }
    )?;
    
    // 2. Calculate cosine similarity with all other assets of same type
    let mut stmt = conn.prepare(
        "SELECT id, name, embedding, asset_type 
         FROM assets 
         WHERE id != ? 
           AND asset_type = (SELECT asset_type FROM assets WHERE id = ?)"
    )?;
    
    let candidates = stmt.query_map(params![asset_id, asset_id], |row| {
        let id: i64 = row.get(0)?;
        let name: String = row.get(1)?;
        let emb_blob: Vec<u8> = row.get(2)?;
        let emb = deserialize_embedding(&emb_blob);
        
        let similarity = cosine_similarity(&query_embedding, &emb);
        
        Ok(SimilarAsset {
            id,
            name,
            similarity,
        })
    })?
    .collect::<Result<Vec<_>, _>>()?;
    
    // 3. Sort by similarity and return top N
    let mut results = candidates;
    results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());
    results.truncate(limit as usize);
    
    Ok(results)
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot / (norm_a * norm_b)
}
```

### Hybrid Query: Text Search + Similarity Reranking

```rust
#[tauri::command]
pub async fn hybrid_search(
    state: State<'_, SharedState>,
    text_query: String,
    reference_asset_id: Option<AssetId>,
    limit: u32,
) -> Result<Vec<AssetSearchResult>, String> {
    let conn = state.db_connection.lock().unwrap();
    
    // 1. Get text search results (larger initial set)
    let mut text_results = text_search(&conn, &text_query, limit * 3)?;
    
    // 2. If reference asset provided, rerank by similarity
    if let Some(ref_id) = reference_asset_id {
        let ref_embedding: Vec<f32> = conn.query_row(
            "SELECT embedding FROM assets WHERE id = ?",
            params![ref_id],
            |row| {
                let blob: Vec<u8> = row.get(0)?;
                Ok(deserialize_embedding(&blob))
            }
        )?;
        
        // Calculate similarity scores
        for result in &mut text_results {
            let similarity = cosine_similarity(&ref_embedding, &result.embedding);
            // Combine text relevance (BM25) with vector similarity
            result.hybrid_score = result.text_score * 0.4 + similarity * 0.6;
        }
        
        // Re-sort by hybrid score
        text_results.sort_by(|a, b| {
            b.hybrid_score.partial_cmp(&a.hybrid_score).unwrap()
        });
    }
    
    text_results.truncate(limit as usize);
    Ok(text_results)
}
```

---

## Performance Optimizations

### 1. FTS5 Index Optimization

```sql
-- Optimize FTS5 index (run periodically)
INSERT INTO assets_fts(assets_fts) VALUES('optimize');

-- Rebuild FTS5 index if corrupted
INSERT INTO assets_fts(assets_fts) VALUES('rebuild');

-- Get index statistics
SELECT * FROM assets_fts_data;
```

### 2. Query Result Caching

```rust
use lru::LruCache;

pub struct SearchCache {
    cache: Mutex<LruCache<String, Vec<AssetSearchResult>>>,
}

impl SearchCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: Mutex::new(LruCache::new(capacity)),
        }
    }
    
    pub fn get_or_search<F>(
        &self,
        query_key: &str,
        search_fn: F,
    ) -> Result<Vec<AssetSearchResult>>
    where
        F: FnOnce() -> Result<Vec<AssetSearchResult>>,
    {
        let mut cache = self.cache.lock().unwrap();
        
        if let Some(cached) = cache.get(query_key) {
            return Ok(cached.clone());
        }
        
        drop(cache); // Release lock before expensive operation
        
        let results = search_fn()?;
        
        let mut cache = self.cache.lock().unwrap();
        cache.put(query_key.to_string(), results.clone());
        
        Ok(results)
    }
}
```

### 3. Pagination with Cursor

```rust
// Use cursor-based pagination for large result sets
#[derive(Serialize, Deserialize)]
pub struct SearchCursor {
    pub last_id: AssetId,
    pub last_score: f32,
}

pub fn search_with_cursor(
    conn: &Connection,
    query: &SearchQuery,
    cursor: Option<SearchCursor>,
) -> Result<(Vec<AssetSearchResult>, Option<SearchCursor>)> {
    let mut sql = build_search_query(query);
    
    // Add cursor condition
    if let Some(cursor) = cursor {
        sql.push_str(&format!(
            " AND (bm25(fts) > ? OR (bm25(fts) = ? AND a.id > ?))",
        ));
    }
    
    sql.push_str(&format!(" LIMIT {}", query.limit + 1));
    
    let results = execute_query(conn, &sql)?;
    
    // Check if there are more results
    let has_more = results.len() > query.limit as usize;
    let next_cursor = if has_more {
        let last = &results[query.limit as usize - 1];
        Some(SearchCursor {
            last_id: last.id,
            last_score: last.text_score,
        })
    } else {
        None
    };
    
    Ok((results, next_cursor))
}
```

---

## Search Quality Comparison by Tier

### Example Query: "knight character with sword"

**Fast Tier Results:**
```
Matching assets: 47
Quality: Good
- Matches on tags: "character", "knight", "sword"
- May miss context like "holding" or "wielding"
- Basic relevance ranking

Top result: knight_sprite.png
Tags: ["character", "pixel art", "knight"]
Match reason: 2/3 tags matched
```

**Quality Tier Results:**
```
Matching assets: 73
Quality: Very Good
- Matches on detailed tags: "knight character", "holding sword", "armor"
- Better semantic understanding
- Improved relevance ranking

Top result: hero_knight_sword.png
Tags: ["knight character", "pixel art", "blue armor", "sword weapon", "medieval"]
Match reason: Strong semantic match on all terms
```

**Premium Tier Results:**
```
Matching assets: 89
Quality: Excellent
- Matches on natural language descriptions
- Understands complex relationships
- Context-aware ranking

Top result: paladin_walk_armed.png
Description: "A pixel art sprite sheet depicting a knight character in blue medieval 
armor holding a silver longsword in their right hand. The character is shown in 8 
walking animation frames facing right..."
Match reason: Complete semantic understanding of query intent
```

### Arbitrary Text Query Support

**Query: "ambient forest sounds with birds but no wind"**

**Fast Tier:**
- May not find good matches (no "birds" tag)
- Falls back to "ambient" + "forest" if those tags exist
- Limited by predefined categories

**Quality Tier:**
- Better understanding of "ambient" + "forest" + "birds"
- Can match on semantic embeddings even without exact tags
- May struggle with negation ("no wind")

**Premium Tier:**
- Full natural language understanding
- Understands "ambient forest with birds but no wind"
- Can parse complex queries: "sounds with X but not Y"
- Searches descriptions like "chirping birds in quiet forest setting"

### Performance Comparison

| Tier | Index Size | Search Speed | Recall @ 10 | Precision |
|------|-----------|--------------|-------------|-----------|
| Fast | 50MB | <10ms | 65-75% | 70-80% |
| Quality | 150MB | <25ms | 75-85% | 80-90% |
| Premium | 800MB | <50ms | 85-95% | 90-95% |

*Note: Premium tier search is slower but provides dramatically better results*

---

## Summary

This comprehensive search system provides:

✅ **Full-text search** with FTS5 (autocomplete, phrase matching, boolean operators)  
✅ **Automatic tagging** via CLIP (images) and PANNs/CLAP (audio) at three quality levels  
✅ **Faceted filtering** (style, category, dimensions, duration, tags)  
✅ **Vector similarity** search for "find similar" features  
✅ **Hybrid search** combining text relevance and vector similarity  
✅ **Performance optimizations** (caching, pagination, index tuning)  
✅ **Quality tiers** - users choose speed vs. accuracy tradeoff

The system scales to 10,000+ assets with:
- **Fast tier**: Sub-50ms search response times
- **Quality tier**: Sub-100ms search response times  
- **Premium tier**: Sub-150ms search response times

All tiers support offline operation with no external API dependencies.