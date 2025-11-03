# Implementation Checklists

Use these checklists to track progress and ensure completeness at each phase.

---

## Phase 1: Foundation

### Backend Setup
- [ ] Create Tauri project structure
- [ ] Set up Cargo workspace
- [ ] Add required dependencies (rusqlite, tokio, tauri, etc.)
- [ ] Create database initialization module
- [ ] Create all required tables (assets, tags, asset_tags, duplicates, FTS5)
- [ ] Test database creation and schema
- [ ] Create models module (Asset, ImportJob, QualityTier, etc.)
- [ ] Create commands module structure
- [ ] Implement `import_assets` command
- [ ] Test import with sample files
- [ ] Set up app state management
- [ ] Create main.rs entry point

### Frontend Setup
- [ ] Initialize SvelteKit project
- [ ] Set up Tailwind CSS 4
- [ ] Create app layout and main page
- [ ] Create types module
- [ ] Create state modules (assets.svelte.ts, ui.svelte.ts)
- [ ] Create shared components directory
- [ ] Implement AssetCard component
- [ ] Implement asset grid with pagination
- [ ] Add import button with file picker
- [ ] Implement quality tier selector
- [ ] Test pagination with 50+ assets
- [ ] Set up CSS variables for theming

### Integration
- [ ] Connect frontend import button to backend
- [ ] Verify data flows from Tauri commands to UI
- [ ] Test asset persistence across app reload
- [ ] Verify asset grid updates after import
- [ ] Test all filter/sort options
- [ ] Check responsive design

### Testing & Validation
- [ ] Import 10 test assets
- [ ] Verify database has correct records
- [ ] Test with different file formats (PNG, JPG, MP3, WAV)
- [ ] Test with ZIP archives
- [ ] Test pagination with 100+ assets
- [ ] Verify no errors in console
- [ ] Check memory usage under load

---

## Phase 2: Search & Filtering

### ML Model Integration
- [ ] Download CLIP ViT-B/32 model
- [ ] Download PANNs CNN14 model
- [ ] Create model manager
- [ ] Set up ONNX Runtime
- [ ] Verify models load without errors
- [ ] Test inference on sample image
- [ ] Test inference on sample audio

### Image Processing
- [ ] Implement CLIP image tagger
- [ ] Test auto-tagging on sample images
- [ ] Implement sprite sheet detection (FFT)
- [ ] Test sprite sheet detection accuracy
- [ ] Generate and store embeddings
- [ ] Create image to tensor conversion
- [ ] Test perceptual hashing
- [ ] Implement LOD thumbnail generation

### Audio Processing
- [ ] Implement audio decoder (symphonia)
- [ ] Implement mel spectrogram generation
- [ ] Implement PANNs audio tagger
- [ ] Test audio tagging on samples
- [ ] Generate and store audio embeddings
- [ ] Implement audio duration categorization
- [ ] Implement chromaprint fingerprinting

### Search System
- [ ] Implement FTS5 search handler
- [ ] Test full-text search (word, phrase, boolean operators)
- [ ] Implement prefix autocomplete
- [ ] Test search with special characters
- [ ] Implement faceted filtering
- [ ] Test style filter
- [ ] Test sound category filter
- [ ] Test audio duration filter
- [ ] Implement BM25 relevance ranking
- [ ] Test sorting options (relevance, name, date, etc.)
- [ ] Implement pagination for search results

### Vector Similarity
- [ ] Implement cosine similarity
- [ ] Implement `find_similar_assets` command
- [ ] Test similarity search accuracy
- [ ] Implement hybrid search (text + vector)
- [ ] Test reranking by similarity

### Frontend Search UI
- [ ] Create SearchBar component
- [ ] Implement filter state management
- [ ] Add debounced search triggering
- [ ] Create style filter chips
- [ ] Create audio category filter chips
- [ ] Create duration dropdown
- [ ] Add sort options
- [ ] Implement results display
- [ ] Show result count and loading state
- [ ] Test all filter combinations
- [ ] Verify responsive design

### Testing & Validation
- [ ] Process 100 assets with CLIP/PANNs
- [ ] Verify tags quality visually
- [ ] Test search with 100+ assets
- [ ] Performance test (search should be <100ms)
- [ ] Test sprite sheet detection (20+ test cases)
- [ ] Verify embedding storage
- [ ] Check vector similarity accuracy

---

## Phase 3: Advanced Features

### Premium Tier ML
- [ ] Download LLaVA 1.6 7B model (INT8 quantized)
- [ ] Download Audio LLM (Qwen2-Audio or SALMONN)
- [ ] Implement LLaVA description generator
- [ ] Implement Audio LLM description generator
- [ ] Test description quality on samples
- [ ] Implement `ask_about_asset` command
- [ ] Test question-answering functionality
- [ ] Implement upgrade_assets_quality command
- [ ] Test reprocessing from fast → quality → premium

### Clustering & Layout
- [ ] Integrate UMAP dimensionality reduction
- [ ] Test UMAP with 1,000 embeddings
- [ ] Integrate HDBSCAN clustering
- [ ] Test clustering quality
- [ ] Implement layout computation
- [ ] Store position_x, position_y, cluster_id in DB
- [ ] Implement `get_canvas_data` command

### Infinite Canvas
- [ ] Set up PixiJS application
- [ ] Implement Viewport for pan/zoom
- [ ] Load and render asset sprites
- [ ] Implement viewport culling (only render visible)
- [ ] Implement LOD switching (small/medium/large)
- [ ] Test rendering 10,000 sprites (60 FPS target)
- [ ] Implement hover interactions
- [ ] Implement click to show details
- [ ] Add keyboard navigation
- [ ] Test zoom performance

### Duplicate Detection
- [ ] Implement perceptual hashing comparison
- [ ] Test image duplicate detection
- [ ] Implement chromaprint fingerprint comparison
- [ ] Test audio duplicate detection
- [ ] Implement embedding-based duplicates
- [ ] Create duplicate grouping algorithm
- [ ] Implement `find_duplicates` command
- [ ] Implement `resolve_duplicate` command
- [ ] Create duplicate resolution UI
- [ ] Test all detection methods

### Performance Optimization
- [ ] Implement thumbnail LRU cache
- [ ] Measure cache hit rates
- [ ] Implement query result caching
- [ ] Profile hot code paths
- [ ] Optimize database indices
- [ ] Test with 10,000 assets
- [ ] Measure VRAM usage per tier
- [ ] Implement lazy loading for large results
- [ ] Optimize asset card rendering

### File Watching
- [ ] Implement directory watcher
- [ ] Test incremental imports
- [ ] Implement debouncing for rapid file changes
- [ ] Auto-process new assets
- [ ] Verify file watching doesn't impact performance

### Settings UI
- [ ] Create settings panel
- [ ] Implement quality tier selector
- [ ] Implement processing mode selector
- [ ] Implement scheduled time picker
- [ ] Create `get_processing_config` command
- [ ] Create `set_processing_config` command
- [ ] Test settings persistence
- [ ] Test scheduled processing

### Error Handling & Feedback
- [ ] Implement comprehensive error messages
- [ ] Create toast notification system
- [ ] Create confirmation dialog component
- [ ] Implement progress tracking for long operations
- [ ] Add job status monitoring
- [ ] Implement job cancellation
- [ ] Test error recovery

### Testing & Validation
- [ ] Process 10,000 assets with Quality tier
- [ ] Process 1,000 assets with Premium tier
- [ ] Verify 60 FPS rendering with 10,000 items
- [ ] Test all tier upgrade paths
- [ ] Verify duplicate detection accuracy
- [ ] Test search with 10,000 assets
- [ ] Performance profile with VRAM monitor
- [ ] Test on target hardware (16GB RAM)
- [ ] Verify no memory leaks
- [ ] Load test with continuous operations

### Documentation & Polish
- [ ] Update README with features
- [ ] Create user guide
- [ ] Add keyboard shortcut help
- [ ] Implement about dialog
- [ ] Add version display
- [ ] Test on multiple machines
- [ ] Verify installer creation
- [ ] Create release notes

---

## Code Quality

### For All Phases
- [ ] No compiler warnings (Rust)
- [ ] No TypeScript errors
- [ ] Format code (`cargo fmt`, `npm run format`)
- [ ] No eslint warnings
- [ ] Add doc comments to public functions
- [ ] Create unit tests for utilities
- [ ] Add integration tests for database
- [ ] Verify error handling on edge cases
- [ ] Test with invalid input data

### Backend (Rust)
- [ ] Use `Result<T, E>` for fallible operations
- [ ] No `.unwrap()` in user-facing code
- [ ] Proper error messages for debugging
- [ ] Use proper thread safety primitives
- [ ] No deadlocks with locks/mutexes
- [ ] Async/await used correctly

### Frontend (Svelte)
- [ ] Use Svelte 5 runes syntax
- [ ] No legacy `$:` reactive statements
- [ ] State exports properly structured
- [ ] No `<style>` blocks in components
- [ ] Use Tailwind utilities
- [ ] Props properly typed
- [ ] Components reusable (not one-off)

---

## Pre-Release Checklist

- [ ] All features working as designed
- [ ] No known critical bugs
- [ ] Performance validated at target scale
- [ ] Memory usage within limits
- [ ] All error cases handled gracefully
- [ ] Documentation complete and accurate
- [ ] User guide created
- [ ] Installer tested on clean system
- [ ] Settings persist across restarts
- [ ] Import/export functionality works
- [ ] Database can be backed up/restored
- [ ] No security vulnerabilities
- [ ] No hardcoded paths or API keys
- [ ] Logging functional for debugging
- [ ] Version number updated
- [ ] Changelog created
- [ ] License information included
- [ ] Third-party credits listed

---

## Verification Commands

### Database Verification
```bash
# Check schema
sqlite3 ~/.config/asseteer/asseteer.db ".schema assets"

# Count assets
sqlite3 ~/.config/asseteer/asseteer.db "SELECT COUNT(*) FROM assets;"

# Check FTS5
sqlite3 ~/.config/asseteer/asseteer.db "SELECT COUNT(*) FROM assets_fts;"

# Verify indices
sqlite3 ~/.config/asseteer/asseteer.db ".indices"
```

### Build Verification
```bash
# Backend checks
cargo check
cargo clippy
cargo test

# Frontend checks
npm run check:svelte
npm run check:vite
npm run build
```

### Performance Verification
```bash
# Profile backend
cargo flamegraph --bin asseteer

# Check VRAM usage
nvidia-smi  # GPU
top -n 1    # System memory
```

### Testing Sample Assets
- **Images**: PNG (32x32), JPEG (1920x1080), WebP, AVIF
- **Audio**: MP3 (128kbps), WAV (44.1kHz), OGG, FLAC
- **Archives**: ZIP with 100+ mixed assets
- **Edge Cases**: Very large (50MB), very small (1KB), corrupted files

---

## Common Issues & Solutions

| Issue | Cause | Solution |
|-------|-------|----------|
| Models not downloading | Network issue | Check URL, retry manually |
| Slow search | Large asset count | Run FTS5 optimize, add index |
| High VRAM | Premium tier | Switch to Quality tier |
| UI freeze | Main thread blocking | Use spawn_blocking for CPU work |
| Database locked | Concurrent access | Use WAL mode (already configured) |
| Duplicates missed | Low threshold | Reduce threshold, try embedding method |
| Canvas flickering | Rendering loop issue | Check FPS, verify culling |
| Tags missing | Processing failed | Check ML model, see logs |

