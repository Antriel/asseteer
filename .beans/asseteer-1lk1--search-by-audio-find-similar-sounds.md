---
# asseteer-1lk1
title: 'Search by audio: find similar sounds'
status: draft
type: feature
priority: normal
created_at: 2026-03-18T09:18:44Z
updated_at: 2026-03-18T09:22:15Z
---

Allow users to find audio assets similar to a selected audio asset by comparing CLAP embeddings directly (audio-to-audio similarity), reusing the existing text-to-audio semantic search infrastructure.

## How It Works

CLAP maps both text and audio into the same 512-dimensional embedding space. Currently we do:
- **Text search**: text → embedding → cosine similarity against all audio embeddings

"Search by audio" would do:
- **Audio search**: pick an audio asset → use its *existing* stored embedding → cosine similarity against all other audio embeddings

No new model inference needed — the embedding is already in the `audio_embeddings` table. This is purely a lookup + vector comparison, so it's instant.

## Backend Changes

### New Tauri command: `search_audio_by_similarity`

In `src-tauri/src/commands/search.rs`, add a new command:

```rust
#[tauri::command]
pub async fn search_audio_by_similarity(
    asset_id: i64,
    limit: usize,
    min_duration_ms: Option<i64>,
    max_duration_ms: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<SemanticSearchResult>, String>
```

**Logic:**
1. Fetch the source asset's embedding from `audio_embeddings` where `asset_id` matches
2. Load all other audio embeddings (same SQL as `search_audio_semantic`, but exclude source asset)
3. Compute cosine similarity for each (reuse existing `cosine_similarity()`)
4. Sort descending, return top `limit` results as `SemanticSearchResult`

This is almost identical to `search_audio_semantic` — the only difference is step 1 fetches an existing embedding instead of generating one from text.

### Frontend query wrapper

In `src/lib/database/queries.ts`, add:

```typescript
export async function searchAudioBySimilarity(
    assetId: number,
    limit: number = 500,
    durationFilter?: DurationFilter,
): Promise<SemanticSearchResult[]>
```

## Frontend / UI Changes

### Entry point: "Find Similar" action

The most natural UX is a context-menu or action-button on any audio asset. Options:

**Option A — Context menu item (recommended)**
- Right-click any audio asset in the list → "Find Similar Sounds"
- Minimal UI footprint, discoverable, consistent with typical asset manager UX

**Option B — Button in the audio player / detail area**
- When an audio file is selected/playing, show a "Find Similar" button
- Could be an icon button (e.g., a "similar" or "related" icon) near the play controls

**Option C — Both** (context menu + player button)

### Search results display

When "Find Similar" is triggered:
1. Switch to semantic search mode (reuse existing `AudioList` with `showSimilarity={true}`)
2. Show a banner/chip at the top: `Similar to: "filename.wav" ×` (with a clear/dismiss button)
3. Results appear sorted by similarity %, using the same layout as text semantic search
4. Clicking the `×` or clearing returns to normal browse mode

### State management

In `src/lib/state/clap.svelte.ts`, add:

```typescript
similarToAssetId: null as number | null,
similarToFilename: null as string | null,

async searchBySimilarity(assetId: number, filename: string, limit = 500, durationFilter?: DurationFilter) {
    // Similar to existing search() but calls searchAudioBySimilarity
    // Sets similarToAssetId/Filename for the UI banner
}

clearSimilaritySearch() {
    this.similarToAssetId = null;
    this.similarToFilename = null;
    this.semanticResults = [];
}
```

### Toolbar integration

When `similarToAssetId` is set:
- Replace the search input with a read-only chip: `Similar to: "filename.wav" ×`
- Or show the chip above/below the search bar
- The duration filter should still work (it's passed through to the backend)

## Implementation Plan

- [ ] Add `search_audio_by_similarity` Tauri command (backend)
- [ ] Add `searchAudioBySimilarity` query wrapper (frontend)
- [ ] Add state management for similarity search mode in `clap.svelte.ts`
- [ ] Add "Find Similar" to audio asset context menu
- [ ] Show "Similar to: filename" banner/chip when in similarity mode
- [ ] Wire up results display (reuse AudioList + similarity %)
- [ ] Handle edge case: asset has no embedding yet (show toast: "This asset hasn't been processed yet")
- [ ] Ensure duration filter works in similarity mode
- [ ] Optional: Add "Find Similar" button in audio player/detail area

## Notes

- No CLAP server needed at search time — embeddings are already stored. The server is only needed if the source asset hasn't been processed yet.
- Performance should be identical to text semantic search (same vector comparison loop).
- The `SemanticSearchResult` struct works as-is — no changes needed.
- Could later extend this to drag-and-drop an external audio file (would need server to generate embedding on the fly), but that's a separate feature.
