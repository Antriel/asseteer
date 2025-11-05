# UI Improvement Plan - Asseteer

**Created**: 2025-11-04
**Status**: Planning Phase

## Executive Summary

This document outlines a comprehensive plan to enhance the Asseteer UI with:
- **Separate viewing modes** for images vs audio assets
- **Improved search and filtering** capabilities
- **Denser, more visual UI** with grid and table layouts
- **Audio playback** functionality with media controls
- **Full-size image viewing** with lightbox/modal
- **Enhanced user experience** with better navigation and organization

---

## Current State Analysis

### Existing Implementation
- **Display Mode**: Table-only view with 5 columns (Preview, Name, Type, Dimensions, Size)
- **Search**: Basic FTS5 full-text search, no filtering by asset type
- **Thumbnails**: 64×64px WebP thumbnails, generated during processing
- **Asset Types**: Mixed display (images and audio shown together)
- **No Media Viewers**: No way to view full-size images or play audio files
- **Layout**: Single-density table layout, not optimized for visual browsing

### Key Files
- `src/routes/+page.svelte` - Main page container
- `src/lib/components/AssetList.svelte` - Asset table and search
- `src/lib/components/AssetThumbnail.svelte` - Thumbnail display
- `src/lib/state/assets.svelte.ts` - Asset state management
- `src/lib/database/queries.ts` - Database query functions

---

## Design Goals

### 1. Asset Type Separation
**Goal**: Users should view images or audio separately, not mixed together.

**Rationale**:
- Different asset types require different UI patterns
- Images benefit from grid/gallery view with large thumbnails
- Audio needs list view with metadata and playback controls
- Mixed views create cognitive overhead and clutter

**UX Flow**:
```
[Images Tab] [Audio Tab]
     ↓
  Images Mode:
  - Large thumbnail grid (3-5 columns)
  - Image-specific filters (dimensions, format)
  - Click to open full-size lightbox

  Audio Mode:
  - Detailed list with waveform icons
  - Audio-specific filters (duration, bitrate)
  - Inline playback controls
```

### 2. Enhanced Search & Filtering
**Goal**: Powerful, context-aware search with multi-dimensional filtering.

**Features**:
- Type-aware search (searches only current tab's assets)
- Advanced filters: date range, file size, format
- Image-specific: dimensions, aspect ratio
- Audio-specific: duration range, sample rate, channels
- Sort options: name, size, date, dimensions/duration

### 3. Denser, More Visual UI
**Goal**: Maximize information density while improving visual hierarchy.

**Principles**:
- Grid view for images (visual browsing)
- Compact table view for audio (metadata-focused)
- Responsive thumbnail sizes (user-configurable)
- Efficient use of screen real estate
- Clear visual separation between sections

### 4. Media Viewing Capabilities
**Goal**: Enable users to consume media directly in the app.

**Image Viewing**:
- Full-screen lightbox modal
- Zoom and pan controls
- Keyboard navigation (arrow keys for prev/next)
- Image metadata overlay

**Audio Playback**:
- Inline player with standard controls (play, pause, scrub)
- Visual progress indicator
- Volume control
- Metadata display (duration, format, bitrate)
- Waveform visualization (optional, Phase 2)

---

## Implementation Plan

## Phase 1: Asset Type Tabs & State Management

### 1.1 Create Tab System

**New State**: `src/lib/state/view.svelte.ts`
```typescript
type AssetViewMode = 'images' | 'audio';
type LayoutMode = 'grid' | 'table';

class ViewState {
  activeTab = $state<AssetViewMode>('images');
  layoutMode = $state<LayoutMode>('grid'); // For images
  thumbnailSize = $state<'small' | 'medium' | 'large'>('medium');

  setActiveTab(tab: AssetViewMode) {
    this.activeTab = tab;
    // Reset layout mode based on tab
    this.layoutMode = tab === 'images' ? 'grid' : 'table';
  }

  toggleLayoutMode() {
    this.layoutMode = this.layoutMode === 'grid' ? 'table' : 'grid';
  }

  setThumbnailSize(size: 'small' | 'medium' | 'large') {
    this.thumbnailSize = size;
  }
}

export const viewState = new ViewState();
```

**Update Assets State**: `src/lib/state/assets.svelte.ts`
```typescript
class AssetsState {
  // ... existing state

  // Add computed asset type filter
  getFilteredAssets(assetType: 'image' | 'audio') {
    return this.assets.filter(a => a.asset_type === assetType);
  }

  // Update loadAssets to accept optional type filter
  async loadAssets(assetType?: 'image' | 'audio') {
    this.isLoading = true;
    const db = await getDatabase();

    this.assets = await searchAssets(
      db,
      this.searchText || undefined,
      assetType, // Pass type filter to query
      this.pageSize,
      this.currentOffset
    );

    this.isLoading = false;
  }

  // Update search to use current tab filter
  searchAssets(text: string, assetType?: 'image' | 'audio') {
    this.searchText = text;
    this.currentOffset = 0;
    this.loadAssets(assetType);
  }
}
```

### 1.2 Create Tab Navigation Component

**New Component**: `src/lib/components/shared/TabBar.svelte`
```svelte
<script lang="ts">
  import { viewState } from '$lib/state/view.svelte';
  import { assetsState } from '$lib/state/assets.svelte';

  interface Props {
    imageCoun: number;
    audioCount: number;
  }

  let { imageCount, audioCount }: Props = $props();

  function switchTab(tab: 'images' | 'audio') {
    viewState.setActiveTab(tab);
    assetsState.loadAssets(tab === 'images' ? 'image' : 'audio');
  }
</script>

<div class="flex items-center gap-1 border-b border-default bg-secondary px-4">
  <button
    class="tab-btn"
    class:active={viewState.activeTab === 'images'}
    onclick={() => switchTab('images')}
  >
    Images
    <span class="count">{imageCount}</span>
  </button>

  <button
    class="tab-btn"
    class:active={viewState.activeTab === 'audio'}
    onclick={() => switchTab('audio')}
  >
    Audio
    <span class="count">{audioCount}</span>
  </button>
</div>

<style>
  .tab-btn {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.75rem 1rem;
    border-bottom: 2px solid transparent;
    font-weight: 500;
    color: var(--color-text-secondary);
    transition: all 0.2s;
  }

  .tab-btn:hover {
    color: var(--color-text-primary);
  }

  .tab-btn.active {
    color: var(--color-accent);
    border-bottom-color: var(--color-accent);
  }

  .count {
    font-size: 0.75rem;
    padding: 0.125rem 0.5rem;
    background: var(--color-bg-tertiary);
    border-radius: 9999px;
  }
</style>
```

### 1.3 Database Query Enhancement

**Update**: `src/lib/database/queries.ts`

Add new queries for asset counts by type:
```typescript
export async function getAssetCountByType(
  db: Database,
  assetType: 'image' | 'audio'
): Promise<number> {
  const result = await db.select<[{ count: number }]>(
    'SELECT COUNT(*) as count FROM assets WHERE asset_type = ?',
    [assetType]
  );
  return result[0]?.count ?? 0;
}

export async function getAssetTypeCounts(
  db: Database
): Promise<{ images: number; audio: number }> {
  const [images, audio] = await Promise.all([
    getAssetCountByType(db, 'image'),
    getAssetCountByType(db, 'audio')
  ]);
  return { images, audio };
}
```

---

## Phase 2: Grid Layout for Images

### 2.1 Create Grid View Component

**New Component**: `src/lib/components/ImageGrid.svelte`
```svelte
<script lang="ts">
  import type { Asset } from '$lib/types';
  import { viewState } from '$lib/state/view.svelte';
  import ImageThumbnail from './ImageThumbnail.svelte';

  interface Props {
    assets: Asset[];
  }

  let { assets }: Props = $props();

  // Computed grid column classes based on thumbnail size
  const gridClasses = $derived(() => {
    switch (viewState.thumbnailSize) {
      case 'small': return 'grid-cols-6 xl:grid-cols-8';
      case 'medium': return 'grid-cols-4 xl:grid-cols-6';
      case 'large': return 'grid-cols-3 xl:grid-cols-4';
    }
  });

  function handleImageClick(asset: Asset) {
    // Open lightbox modal (Phase 3)
  }
</script>

<div class="grid {gridClasses()} gap-2 p-4">
  {#each assets as asset (asset.id)}
    <button
      class="image-card"
      onclick={() => handleImageClick(asset)}
    >
      <ImageThumbnail assetId={asset.id} size={viewState.thumbnailSize} />

      <div class="image-info">
        <p class="filename" title={asset.filename}>{asset.filename}</p>
        {#if asset.width && asset.height}
          <p class="dimensions">{asset.width} × {asset.height}</p>
        {/if}
      </div>
    </button>
  {/each}
</div>

<style>
  .image-card {
    position: relative;
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border-default);
    border-radius: 8px;
    overflow: hidden;
    transition: all 0.2s;
    cursor: pointer;
  }

  .image-card:hover {
    border-color: var(--color-accent);
    box-shadow: 0 4px 8px rgba(0, 0, 0, 0.1);
    transform: translateY(-2px);
  }

  .image-info {
    padding: 0.5rem;
    background: var(--color-bg-primary);
  }

  .filename {
    font-size: 0.75rem;
    font-weight: 500;
    color: var(--color-text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .dimensions {
    font-size: 0.625rem;
    color: var(--color-text-secondary);
    margin-top: 0.25rem;
  }
</style>
```

### 2.2 Enhanced Thumbnail Component

**New Component**: `src/lib/components/ImageThumbnail.svelte`

Replace existing `AssetThumbnail.svelte` for images with larger, responsive thumbnails:

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import { getDatabase } from '$lib/database/connection';
  import { getThumbnail } from '$lib/database/queries';

  interface Props {
    assetId: number;
    size?: 'small' | 'medium' | 'large';
  }

  let { assetId, size = 'medium' }: Props = $props();

  let thumbnailUrl = $state<string | null>(null);
  let isLoading = $state(true);
  let error = $state<string | null>(null);

  const sizeClasses = $derived(() => {
    switch (size) {
      case 'small': return 'h-32';
      case 'medium': return 'h-48';
      case 'large': return 'h-64';
    }
  });

  onMount(async () => {
    try {
      const db = await getDatabase();
      const thumbnailData = await getThumbnail(db, assetId);

      if (thumbnailData) {
        const blob = new Blob([thumbnailData], { type: 'image/webp' });
        thumbnailUrl = URL.createObjectURL(blob);
      }
    } catch (e) {
      error = String(e);
    } finally {
      isLoading = false;
    }

    return () => {
      if (thumbnailUrl) {
        URL.revokeObjectURL(thumbnailUrl);
      }
    };
  });
</script>

<div class="thumbnail-container {sizeClasses()}">
  {#if isLoading}
    <div class="placeholder">
      <div class="spinner"></div>
    </div>
  {:else if error || !thumbnailUrl}
    <div class="placeholder">
      <span class="text-xs text-secondary">No preview</span>
    </div>
  {:else}
    <img
      src={thumbnailUrl}
      alt="Thumbnail"
      class="thumbnail-image"
    />
  {/if}
</div>

<style>
  .thumbnail-container {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--color-bg-tertiary);
    overflow: hidden;
  }

  .thumbnail-image {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .placeholder {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
  }

  .spinner {
    width: 20px;
    height: 20px;
    border: 2px solid var(--color-border-default);
    border-top-color: var(--color-accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }
</style>
```

### 2.3 View Mode Toggle

**New Component**: `src/lib/components/shared/ViewModeToggle.svelte`
```svelte
<script lang="ts">
  import { viewState } from '$lib/state/view.svelte';

  // Only show for images tab
  let showToggle = $derived(viewState.activeTab === 'images');
</script>

{#if showToggle}
  <div class="flex items-center gap-2">
    <button
      class="view-btn"
      class:active={viewState.layoutMode === 'grid'}
      onclick={() => viewState.layoutMode = 'grid'}
      title="Grid View"
    >
      <svg><!-- Grid icon --></svg>
    </button>

    <button
      class="view-btn"
      class:active={viewState.layoutMode === 'table'}
      onclick={() => viewState.layoutMode = 'table'}
      title="Table View"
    >
      <svg><!-- Table icon --></svg>
    </button>

    <!-- Thumbnail size slider (for grid mode only) -->
    {#if viewState.layoutMode === 'grid'}
      <div class="size-control">
        <button onclick={() => viewState.setThumbnailSize('small')}>S</button>
        <button onclick={() => viewState.setThumbnailSize('medium')}>M</button>
        <button onclick={() => viewState.setThumbnailSize('large')}>L</button>
      </div>
    {/if}
  </div>
{/if}
```

---

## Phase 3: Image Lightbox Viewer

### 3.1 Create Lightbox Modal Component

**New Component**: `src/lib/components/modals/ImageLightbox.svelte`

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import type { Asset } from '$lib/types';

  interface Props {
    asset: Asset;
    onClose: () => void;
    onNext?: () => void;
    onPrev?: () => void;
  }

  let { asset, onClose, onNext, onPrev }: Props = $props();

  let zoom = $state(1);
  let showMetadata = $state(false);

  function handleKeydown(e: KeyboardEvent) {
    switch (e.key) {
      case 'Escape':
        onClose();
        break;
      case 'ArrowLeft':
        onPrev?.();
        break;
      case 'ArrowRight':
        onNext?.();
        break;
      case '+':
      case '=':
        zoom = Math.min(zoom + 0.5, 5);
        break;
      case '-':
        zoom = Math.max(zoom - 0.5, 0.5);
        break;
      case '0':
        zoom = 1;
        break;
    }
  }

  onMount(() => {
    document.addEventListener('keydown', handleKeydown);
    return () => document.removeEventListener('keydown', handleKeydown);
  });
</script>

<div class="lightbox-overlay" onclick={onClose}>
  <div class="lightbox-content" onclick={(e) => e.stopPropagation()}>
    <!-- Close button -->
    <button class="close-btn" onclick={onClose}>×</button>

    <!-- Navigation -->
    {#if onPrev}
      <button class="nav-btn nav-prev" onclick={onPrev}>‹</button>
    {/if}
    {#if onNext}
      <button class="nav-btn nav-next" onclick={onNext}>›</button>
    {/if}

    <!-- Image display -->
    <div class="image-container">
      <img
        src={asset.path}
        alt={asset.filename}
        style="transform: scale({zoom})"
        class="lightbox-image"
      />
    </div>

    <!-- Controls -->
    <div class="controls-bar">
      <div class="info">
        <p class="filename">{asset.filename}</p>
        {#if asset.width && asset.height}
          <p class="metadata">{asset.width} × {asset.height} • {(asset.file_size / 1024).toFixed(0)} KB</p>
        {/if}
      </div>

      <div class="actions">
        <button onclick={() => zoom = Math.max(zoom - 0.5, 0.5)}>−</button>
        <span>{Math.round(zoom * 100)}%</span>
        <button onclick={() => zoom = Math.min(zoom + 0.5, 5)}>+</button>
        <button onclick={() => zoom = 1}>Reset</button>
        <button onclick={() => showMetadata = !showMetadata}>Info</button>
      </div>
    </div>

    <!-- Metadata panel -->
    {#if showMetadata}
      <div class="metadata-panel">
        <h3>Image Details</h3>
        <dl>
          <dt>Filename:</dt>
          <dd>{asset.filename}</dd>

          <dt>Path:</dt>
          <dd>{asset.path}</dd>

          {#if asset.width && asset.height}
            <dt>Dimensions:</dt>
            <dd>{asset.width} × {asset.height} px</dd>
          {/if}

          <dt>Format:</dt>
          <dd>{asset.format.toUpperCase()}</dd>

          <dt>File Size:</dt>
          <dd>{(asset.file_size / 1024).toFixed(2)} KB</dd>
        </dl>
      </div>
    {/if}
  </div>
</div>

<style>
  .lightbox-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.9);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .lightbox-content {
    position: relative;
    width: 90vw;
    height: 90vh;
    display: flex;
    flex-direction: column;
  }

  .image-container {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: auto;
  }

  .lightbox-image {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    transition: transform 0.2s;
  }

  .close-btn {
    position: absolute;
    top: 1rem;
    right: 1rem;
    width: 3rem;
    height: 3rem;
    background: rgba(0, 0, 0, 0.5);
    color: white;
    border: none;
    border-radius: 50%;
    font-size: 2rem;
    cursor: pointer;
    z-index: 10;
  }

  .nav-btn {
    position: absolute;
    top: 50%;
    transform: translateY(-50%);
    width: 3rem;
    height: 3rem;
    background: rgba(0, 0, 0, 0.5);
    color: white;
    border: none;
    border-radius: 50%;
    font-size: 2rem;
    cursor: pointer;
  }

  .nav-prev { left: 1rem; }
  .nav-next { right: 1rem; }

  .controls-bar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem;
    background: rgba(0, 0, 0, 0.8);
    color: white;
  }

  .actions {
    display: flex;
    gap: 0.5rem;
    align-items: center;
  }

  .metadata-panel {
    position: absolute;
    top: 4rem;
    right: 1rem;
    width: 300px;
    padding: 1rem;
    background: rgba(0, 0, 0, 0.9);
    color: white;
    border-radius: 8px;
  }
</style>
```

### 3.2 Lightbox State Management

**Update**: `src/lib/state/view.svelte.ts`

```typescript
class ViewState {
  // ... existing state

  // Lightbox state
  lightboxAsset = $state<Asset | null>(null);
  lightboxIndex = $state(0);

  openLightbox(asset: Asset, index: number) {
    this.lightboxAsset = asset;
    this.lightboxIndex = index;
  }

  closeLightbox() {
    this.lightboxAsset = null;
  }

  nextImage(assets: Asset[]) {
    if (this.lightboxIndex < assets.length - 1) {
      this.lightboxIndex++;
      this.lightboxAsset = assets[this.lightboxIndex];
    }
  }

  prevImage(assets: Asset[]) {
    if (this.lightboxIndex > 0) {
      this.lightboxIndex--;
      this.lightboxAsset = assets[this.lightboxIndex];
    }
  }
}
```

---

## Phase 4: Audio List & Player

### 4.1 Create Audio List Component

**New Component**: `src/lib/components/AudioList.svelte`

```svelte
<script lang="ts">
  import type { Asset } from '$lib/types';
  import AudioPlayer from './AudioPlayer.svelte';

  interface Props {
    assets: Asset[];
  }

  let { assets }: Props = $props();

  let currentlyPlaying = $state<number | null>(null);

  function formatDuration(ms: number): string {
    const seconds = Math.floor(ms / 1000);
    const minutes = Math.floor(seconds / 60);
    const remainingSeconds = seconds % 60;
    return `${minutes}:${remainingSeconds.toString().padStart(2, '0')}`;
  }

  function formatFileSize(bytes: number): string {
    return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
  }
</script>

<div class="audio-list">
  {#each assets as asset (asset.id)}
    <div class="audio-item" class:playing={currentlyPlaying === asset.id}>
      <!-- Audio icon/waveform placeholder -->
      <div class="audio-icon">
        <svg><!-- Music note icon --></svg>
      </div>

      <!-- Audio metadata -->
      <div class="audio-info">
        <p class="audio-title">{asset.filename}</p>
        <div class="audio-metadata">
          {#if asset.duration_ms}
            <span>{formatDuration(asset.duration_ms)}</span>
          {/if}
          {#if asset.sample_rate}
            <span>{asset.sample_rate / 1000} kHz</span>
          {/if}
          {#if asset.channels}
            <span>{asset.channels === 1 ? 'Mono' : 'Stereo'}</span>
          {/if}
          <span>{asset.format.toUpperCase()}</span>
          <span>{formatFileSize(asset.file_size)}</span>
        </div>
      </div>

      <!-- Inline player -->
      <div class="player-container">
        <AudioPlayer
          audioPath={asset.path}
          isActive={currentlyPlaying === asset.id}
          onPlay={() => currentlyPlaying = asset.id}
          onPause={() => currentlyPlaying = null}
        />
      </div>
    </div>
  {/each}
</div>

<style>
  .audio-list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    padding: 1rem;
  }

  .audio-item {
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 1rem;
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border-default);
    border-radius: 8px;
    transition: all 0.2s;
  }

  .audio-item:hover {
    border-color: var(--color-accent);
  }

  .audio-item.playing {
    background: var(--color-bg-accent-light);
    border-color: var(--color-accent);
  }

  .audio-icon {
    width: 48px;
    height: 48px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--color-bg-primary);
    border-radius: 8px;
  }

  .audio-info {
    flex: 1;
    min-width: 0;
  }

  .audio-title {
    font-weight: 600;
    color: var(--color-text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .audio-metadata {
    display: flex;
    gap: 1rem;
    margin-top: 0.25rem;
    font-size: 0.75rem;
    color: var(--color-text-secondary);
  }

  .player-container {
    width: 300px;
  }
</style>
```

### 4.2 Create Audio Player Component

**New Component**: `src/lib/components/AudioPlayer.svelte`

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import { convertFileSrc } from '@tauri-apps/api/core';

  interface Props {
    audioPath: string;
    isActive?: boolean;
    onPlay?: () => void;
    onPause?: () => void;
  }

  let { audioPath, isActive = false, onPlay, onPause }: Props = $props();

  let audioElement: HTMLAudioElement;
  let isPlaying = $state(false);
  let currentTime = $state(0);
  let duration = $state(0);
  let volume = $state(1);

  // Convert file path to Tauri-compatible URL
  const audioSrc = $derived(convertFileSrc(audioPath));

  function togglePlay() {
    if (isPlaying) {
      audioElement.pause();
      onPause?.();
    } else {
      audioElement.play();
      onPlay?.();
    }
    isPlaying = !isPlaying;
  }

  function handleTimeUpdate() {
    currentTime = audioElement.currentTime;
  }

  function handleLoadedMetadata() {
    duration = audioElement.duration;
  }

  function handleEnded() {
    isPlaying = false;
    onPause?.();
  }

  function seek(e: MouseEvent) {
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const x = e.clientX - rect.left;
    const percentage = x / rect.width;
    audioElement.currentTime = percentage * duration;
  }

  function formatTime(seconds: number): string {
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return `${mins}:${secs.toString().padStart(2, '0')}`;
  }

  // Pause if another player becomes active
  $effect(() => {
    if (!isActive && isPlaying) {
      audioElement.pause();
      isPlaying = false;
    }
  });
</script>

<div class="audio-player">
  <audio
    bind:this={audioElement}
    src={audioSrc}
    ontimeupdate={handleTimeUpdate}
    onloadedmetadata={handleLoadedMetadata}
    onended={handleEnded}
  />

  <!-- Play/Pause button -->
  <button class="play-btn" onclick={togglePlay}>
    {#if isPlaying}
      <svg><!-- Pause icon --></svg>
    {:else}
      <svg><!-- Play icon --></svg>
    {/if}
  </button>

  <!-- Progress bar -->
  <div class="progress-container" onclick={seek}>
    <div class="progress-bar">
      <div class="progress-fill" style="width: {(currentTime / duration) * 100}%"></div>
    </div>
    <div class="time-display">
      <span>{formatTime(currentTime)}</span>
      <span>{formatTime(duration)}</span>
    </div>
  </div>

  <!-- Volume control -->
  <div class="volume-control">
    <svg><!-- Volume icon --></svg>
    <input
      type="range"
      min="0"
      max="1"
      step="0.1"
      bind:value={volume}
      oninput={() => audioElement.volume = volume}
    />
  </div>
</div>

<style>
  .audio-player {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .play-btn {
    width: 32px;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--color-accent);
    color: white;
    border: none;
    border-radius: 50%;
    cursor: pointer;
  }

  .progress-container {
    flex: 1;
    cursor: pointer;
  }

  .progress-bar {
    height: 4px;
    background: var(--color-border-default);
    border-radius: 2px;
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: var(--color-accent);
    transition: width 0.1s;
  }

  .time-display {
    display: flex;
    justify-content: space-between;
    margin-top: 0.25rem;
    font-size: 0.625rem;
    color: var(--color-text-secondary);
  }

  .volume-control {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .volume-control input[type="range"] {
    width: 60px;
  }
</style>
```

---

## Phase 5: Advanced Search & Filtering

### 5.1 Create Filter Panel Component

**New Component**: `src/lib/components/shared/FilterPanel.svelte`

```svelte
<script lang="ts">
  import { viewState } from '$lib/state/view.svelte';
  import { filterState } from '$lib/state/filters.svelte';

  let isExpanded = $state(false);

  // Show different filters based on active tab
  let showImageFilters = $derived(viewState.activeTab === 'images');
  let showAudioFilters = $derived(viewState.activeTab === 'audio');
</script>

<div class="filter-panel" class:expanded={isExpanded}>
  <button class="filter-toggle" onclick={() => isExpanded = !isExpanded}>
    <svg><!-- Filter icon --></svg>
    Filters
    {#if filterState.hasActiveFilters}
      <span class="badge">{filterState.activeFilterCount}</span>
    {/if}
  </button>

  {#if isExpanded}
    <div class="filter-content">
      <!-- Common filters -->
      <div class="filter-section">
        <h4>File Format</h4>
        {#if showImageFilters}
          <div class="checkbox-group">
            <label><input type="checkbox" bind:checked={filterState.formats.jpg}> JPEG</label>
            <label><input type="checkbox" bind:checked={filterState.formats.png}> PNG</label>
            <label><input type="checkbox" bind:checked={filterState.formats.webp}> WebP</label>
            <label><input type="checkbox" bind:checked={filterState.formats.gif}> GIF</label>
          </div>
        {:else if showAudioFilters}
          <div class="checkbox-group">
            <label><input type="checkbox" bind:checked={filterState.formats.mp3}> MP3</label>
            <label><input type="checkbox" bind:checked={filterState.formats.wav}> WAV</label>
            <label><input type="checkbox" bind:checked={filterState.formats.flac}> FLAC</label>
            <label><input type="checkbox" bind:checked={filterState.formats.ogg}> OGG</label>
          </div>
        {/if}
      </div>

      <!-- File size filter -->
      <div class="filter-section">
        <h4>File Size</h4>
        <div class="range-inputs">
          <input type="number" placeholder="Min (MB)" bind:value={filterState.fileSizeMin} />
          <input type="number" placeholder="Max (MB)" bind:value={filterState.fileSizeMax} />
        </div>
      </div>

      <!-- Image-specific filters -->
      {#if showImageFilters}
        <div class="filter-section">
          <h4>Dimensions</h4>
          <div class="range-inputs">
            <input type="number" placeholder="Min Width" bind:value={filterState.widthMin} />
            <input type="number" placeholder="Max Width" bind:value={filterState.widthMax} />
          </div>
          <div class="range-inputs">
            <input type="number" placeholder="Min Height" bind:value={filterState.heightMin} />
            <input type="number" placeholder="Max Height" bind:value={filterState.heightMax} />
          </div>
        </div>

        <div class="filter-section">
          <h4>Aspect Ratio</h4>
          <select bind:value={filterState.aspectRatio}>
            <option value="">Any</option>
            <option value="square">Square (1:1)</option>
            <option value="landscape">Landscape (16:9)</option>
            <option value="portrait">Portrait (9:16)</option>
          </select>
        </div>
      {/if}

      <!-- Audio-specific filters -->
      {#if showAudioFilters}
        <div class="filter-section">
          <h4>Duration</h4>
          <div class="range-inputs">
            <input type="number" placeholder="Min (sec)" bind:value={filterState.durationMin} />
            <input type="number" placeholder="Max (sec)" bind:value={filterState.durationMax} />
          </div>
        </div>

        <div class="filter-section">
          <h4>Quality</h4>
          <select bind:value={filterState.sampleRate}>
            <option value="">Any Sample Rate</option>
            <option value="44100">44.1 kHz (CD Quality)</option>
            <option value="48000">48 kHz (Studio)</option>
            <option value="96000">96 kHz (High-Res)</option>
          </select>
        </div>
      {/if}

      <!-- Actions -->
      <div class="filter-actions">
        <button class="btn btn-primary" onclick={() => filterState.apply()}>Apply</button>
        <button class="btn btn-secondary" onclick={() => filterState.reset()}>Reset</button>
      </div>
    </div>
  {/if}
</div>
```

### 5.2 Create Filter State

**New State**: `src/lib/state/filters.svelte.ts`

```typescript
class FilterState {
  // Common filters
  formats = $state({
    jpg: true,
    png: true,
    webp: true,
    gif: true,
    mp3: true,
    wav: true,
    flac: true,
    ogg: true
  });

  fileSizeMin = $state<number | null>(null);
  fileSizeMax = $state<number | null>(null);

  // Image-specific
  widthMin = $state<number | null>(null);
  widthMax = $state<number | null>(null);
  heightMin = $state<number | null>(null);
  heightMax = $state<number | null>(null);
  aspectRatio = $state<string>('');

  // Audio-specific
  durationMin = $state<number | null>(null);
  durationMax = $state<number | null>(null);
  sampleRate = $state<string>('');

  get hasActiveFilters(): boolean {
    return this.activeFilterCount > 0;
  }

  get activeFilterCount(): number {
    let count = 0;

    // Count active format filters
    const allFormats = Object.values(this.formats);
    if (!allFormats.every(v => v)) {
      count += allFormats.filter(v => v).length;
    }

    // Count range filters
    if (this.fileSizeMin !== null) count++;
    if (this.fileSizeMax !== null) count++;
    if (this.widthMin !== null) count++;
    if (this.widthMax !== null) count++;
    if (this.heightMin !== null) count++;
    if (this.heightMax !== null) count++;
    if (this.durationMin !== null) count++;
    if (this.durationMax !== null) count++;

    // Count select filters
    if (this.aspectRatio) count++;
    if (this.sampleRate) count++;

    return count;
  }

  apply() {
    // Trigger asset reload with filters
    assetsState.loadAssetsWithFilters(this.buildFilterQuery());
  }

  reset() {
    // Reset all filters
    Object.keys(this.formats).forEach(key => {
      this.formats[key as keyof typeof this.formats] = true;
    });
    this.fileSizeMin = null;
    this.fileSizeMax = null;
    this.widthMin = null;
    this.widthMax = null;
    this.heightMin = null;
    this.heightMax = null;
    this.aspectRatio = '';
    this.durationMin = null;
    this.durationMax = null;
    this.sampleRate = '';

    // Reload without filters
    assetsState.loadAssets();
  }

  buildFilterQuery(): FilterQuery {
    return {
      formats: Object.entries(this.formats)
        .filter(([_, enabled]) => enabled)
        .map(([format, _]) => format),
      fileSizeMin: this.fileSizeMin,
      fileSizeMax: this.fileSizeMax,
      widthMin: this.widthMin,
      widthMax: this.widthMax,
      heightMin: this.heightMin,
      heightMax: this.heightMax,
      aspectRatio: this.aspectRatio,
      durationMin: this.durationMin,
      durationMax: this.durationMax,
      sampleRate: this.sampleRate
    };
  }
}

export const filterState = new FilterState();
```

### 5.3 Update Database Queries

**Update**: `src/lib/database/queries.ts`

Add support for advanced filtering:

```typescript
export interface FilterQuery {
  formats?: string[];
  fileSizeMin?: number | null;
  fileSizeMax?: number | null;
  widthMin?: number | null;
  widthMax?: number | null;
  heightMin?: number | null;
  heightMax?: number | null;
  aspectRatio?: string;
  durationMin?: number | null;
  durationMax?: number | null;
  sampleRate?: string;
}

export async function searchAssetsWithFilters(
  db: Database,
  searchText?: string,
  assetType?: 'image' | 'audio',
  filters?: FilterQuery,
  limit: number = 100,
  offset: number = 0
): Promise<Asset[]> {
  let query = `
    SELECT
      assets.*,
      image_metadata.width,
      image_metadata.height,
      audio_metadata.duration_ms,
      audio_metadata.sample_rate,
      audio_metadata.channels
    FROM assets
    LEFT JOIN image_metadata ON assets.id = image_metadata.asset_id
    LEFT JOIN audio_metadata ON assets.id = audio_metadata.asset_id
  `;

  const conditions: string[] = [];
  const params: any[] = [];

  // Asset type filter
  if (assetType) {
    conditions.push('assets.asset_type = ?');
    params.push(assetType);
  }

  // Search text via FTS5
  if (searchText) {
    query = `
      ${query}
      JOIN assets_fts ON assets.id = assets_fts.rowid
    `;
    conditions.push('assets_fts MATCH ?');
    params.push(searchText + '*');
  }

  // Format filter
  if (filters?.formats && filters.formats.length > 0) {
    conditions.push(`assets.format IN (${filters.formats.map(() => '?').join(', ')})`);
    params.push(...filters.formats);
  }

  // File size filter
  if (filters?.fileSizeMin !== null && filters?.fileSizeMin !== undefined) {
    conditions.push('assets.file_size >= ?');
    params.push(filters.fileSizeMin * 1024 * 1024);
  }
  if (filters?.fileSizeMax !== null && filters?.fileSizeMax !== undefined) {
    conditions.push('assets.file_size <= ?');
    params.push(filters.fileSizeMax * 1024 * 1024);
  }

  // Image-specific filters
  if (assetType === 'image') {
    if (filters?.widthMin !== null && filters?.widthMin !== undefined) {
      conditions.push('image_metadata.width >= ?');
      params.push(filters.widthMin);
    }
    if (filters?.widthMax !== null && filters?.widthMax !== undefined) {
      conditions.push('image_metadata.width <= ?');
      params.push(filters.widthMax);
    }
    if (filters?.heightMin !== null && filters?.heightMin !== undefined) {
      conditions.push('image_metadata.height >= ?');
      params.push(filters.heightMin);
    }
    if (filters?.heightMax !== null && filters?.heightMax !== undefined) {
      conditions.push('image_metadata.height <= ?');
      params.push(filters.heightMax);
    }

    // Aspect ratio
    if (filters?.aspectRatio) {
      switch (filters.aspectRatio) {
        case 'square':
          conditions.push('ABS(image_metadata.width - image_metadata.height) < 10');
          break;
        case 'landscape':
          conditions.push('image_metadata.width > image_metadata.height * 1.5');
          break;
        case 'portrait':
          conditions.push('image_metadata.height > image_metadata.width * 1.5');
          break;
      }
    }
  }

  // Audio-specific filters
  if (assetType === 'audio') {
    if (filters?.durationMin !== null && filters?.durationMin !== undefined) {
      conditions.push('audio_metadata.duration_ms >= ?');
      params.push(filters.durationMin * 1000);
    }
    if (filters?.durationMax !== null && filters?.durationMax !== undefined) {
      conditions.push('audio_metadata.duration_ms <= ?');
      params.push(filters.durationMax * 1000);
    }
    if (filters?.sampleRate) {
      conditions.push('audio_metadata.sample_rate = ?');
      params.push(parseInt(filters.sampleRate));
    }
  }

  // Add WHERE clause
  if (conditions.length > 0) {
    query += ' WHERE ' + conditions.join(' AND ');
  }

  // Add ordering and pagination
  query += ' ORDER BY assets.filename COLLATE NOCASE ASC LIMIT ? OFFSET ?';
  params.push(limit, offset);

  const results = await db.select<Asset[]>(query, params);
  return results;
}
```

---

## Phase 6: UI Polish & Density Improvements

### 6.1 Enhanced Toolbar

**New Component**: `src/lib/components/shared/Toolbar.svelte`

Consolidate search, filters, view controls into a dense toolbar:

```svelte
<script lang="ts">
  import { assetsState } from '$lib/state/assets.svelte';
  import { viewState } from '$lib/state/view.svelte';
  import ViewModeToggle from './ViewModeToggle.svelte';
  import FilterPanel from './FilterPanel.svelte';
  import SortDropdown from './SortDropdown.svelte';

  let searchInput = $state('');

  function handleSearch(e: Event) {
    const value = (e.target as HTMLInputElement).value;
    searchInput = value;
    assetsState.searchAssets(value, viewState.activeTab === 'images' ? 'image' : 'audio');
  }
</script>

<div class="toolbar">
  <!-- Search -->
  <div class="search-container">
    <svg><!-- Search icon --></svg>
    <input
      type="text"
      placeholder="Search {viewState.activeTab}..."
      value={searchInput}
      oninput={handleSearch}
      class="search-input"
    />
  </div>

  <!-- Filters -->
  <FilterPanel />

  <!-- Sort -->
  <SortDropdown />

  <!-- View mode toggle (images only) -->
  <ViewModeToggle />

  <!-- Stats -->
  <div class="stats">
    <span class="text-sm text-secondary">
      {assetsState.assets.length} of {assetsState.totalCount} assets
    </span>
  </div>
</div>

<style>
  .toolbar {
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 0.75rem 1rem;
    background: var(--color-bg-secondary);
    border-bottom: 1px solid var(--color-border-default);
  }

  .search-container {
    position: relative;
    flex: 1;
    max-width: 400px;
  }

  .search-input {
    width: 100%;
    padding: 0.5rem 0.5rem 0.5rem 2rem;
    border: 1px solid var(--color-border-default);
    border-radius: 6px;
    background: var(--color-bg-primary);
    color: var(--color-text-primary);
  }

  .stats {
    margin-left: auto;
  }
</style>
```

### 6.2 Compact Table Layout

**Update**: `src/lib/components/AssetList.svelte`

Improve table density:

```css
/* Add to component styles or app.css */
.asset-table {
  font-size: 0.875rem;
  line-height: 1.25rem;
}

.asset-table thead th {
  padding: 0.5rem 0.75rem;
  font-weight: 600;
  text-transform: uppercase;
  font-size: 0.75rem;
  letter-spacing: 0.05em;
}

.asset-table tbody td {
  padding: 0.5rem 0.75rem;
  border-top: 1px solid var(--color-border-light);
}

.asset-table tbody tr:hover {
  background: var(--color-bg-tertiary);
}
```

### 6.3 Responsive Sizing

Add CSS variables for user-adjustable density:

```css
/* app.css */
:root {
  --thumbnail-size-small: 128px;
  --thumbnail-size-medium: 192px;
  --thumbnail-size-large: 256px;
  --grid-gap: 0.5rem;
  --list-item-height: 64px;
}

/* Density classes */
.density-compact {
  --thumbnail-size-small: 96px;
  --thumbnail-size-medium: 144px;
  --thumbnail-size-large: 192px;
  --grid-gap: 0.25rem;
  --list-item-height: 48px;
}

.density-comfortable {
  --thumbnail-size-small: 128px;
  --thumbnail-size-medium: 192px;
  --thumbnail-size-large: 256px;
  --grid-gap: 0.5rem;
  --list-item-height: 64px;
}

.density-spacious {
  --thumbnail-size-small: 160px;
  --thumbnail-size-medium: 240px;
  --thumbnail-size-large: 320px;
  --grid-gap: 1rem;
  --list-item-height: 80px;
}
```

---

## Phase 7: Integration & Main Layout Update

### 7.1 Update Main Page

**Update**: `src/routes/+page.svelte`

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import { assetsState } from '$lib/state/assets.svelte';
  import { viewState } from '$lib/state/view.svelte';
  import { getDatabase } from '$lib/database/connection';
  import { getAssetTypeCounts } from '$lib/database/queries';

  import ScanControl from '$lib/components/ScanControl.svelte';
  import TaskProgress from '$lib/components/TaskProgress.svelte';
  import TabBar from '$lib/components/shared/TabBar.svelte';
  import Toolbar from '$lib/components/shared/Toolbar.svelte';
  import ImageGrid from '$lib/components/ImageGrid.svelte';
  import AudioList from '$lib/components/AudioList.svelte';
  import ImageLightbox from '$lib/components/modals/ImageLightbox.svelte';

  let assetCounts = $state({ images: 0, audio: 0 });

  onMount(async () => {
    const db = await getDatabase();
    assetCounts = await getAssetTypeCounts(db);

    // Load initial assets (images by default)
    assetsState.loadAssets('image');
  });

  // Filtered assets based on active tab
  let displayedAssets = $derived(
    viewState.activeTab === 'images'
      ? assetsState.assets.filter(a => a.asset_type === 'image')
      : assetsState.assets.filter(a => a.asset_type === 'audio')
  );
</script>

<div class="app-layout">
  <!-- Header -->
  <header class="app-header">
    <h1 class="text-xl font-bold text-primary">Asset Manager</h1>
  </header>

  <!-- Scan Control -->
  <ScanControl />

  <!-- Task Progress -->
  <TaskProgress />

  <!-- Tab Navigation -->
  <TabBar imageCount={assetCounts.images} audioCount={assetCounts.audio} />

  <!-- Toolbar (search, filters, view controls) -->
  <Toolbar />

  <!-- Main Content Area -->
  <main class="content-area">
    {#if assetsState.isLoading}
      <div class="loading-state">
        <div class="spinner"></div>
        <p>Loading assets...</p>
      </div>
    {:else if displayedAssets.length === 0}
      <div class="empty-state">
        <svg><!-- Empty icon --></svg>
        <p>No {viewState.activeTab} found</p>
        <p class="text-sm text-secondary">Try adjusting your search or filters</p>
      </div>
    {:else}
      {#if viewState.activeTab === 'images'}
        {#if viewState.layoutMode === 'grid'}
          <ImageGrid assets={displayedAssets} />
        {:else}
          <!-- Table view for images -->
        {/if}
      {:else}
        <AudioList assets={displayedAssets} />
      {/if}
    {/if}
  </main>

  <!-- Lightbox Modal -->
  {#if viewState.lightboxAsset}
    <ImageLightbox
      asset={viewState.lightboxAsset}
      onClose={() => viewState.closeLightbox()}
      onNext={() => viewState.nextImage(displayedAssets)}
      onPrev={() => viewState.prevImage(displayedAssets)}
    />
  {/if}
</div>

<style>
  .app-layout {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--color-bg-primary);
  }

  .app-header {
    padding: 1rem 1.5rem;
    border-bottom: 1px solid var(--color-border-default);
    background: var(--color-bg-secondary);
  }

  .content-area {
    flex: 1;
    overflow-y: auto;
    position: relative;
  }

  .loading-state,
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    gap: 1rem;
  }

  .spinner {
    width: 40px;
    height: 40px;
    border: 3px solid var(--color-border-default);
    border-top-color: var(--color-accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }
</style>
```

---

## Implementation Checklist

### Phase 1: Asset Type Tabs
- [ ] Create `src/lib/state/view.svelte.ts` with tab and view mode state
- [ ] Update `src/lib/state/assets.svelte.ts` to support type filtering
- [ ] Create `TabBar.svelte` component
- [ ] Add `getAssetTypeCounts` query to `queries.ts`
- [ ] Test tab switching and asset filtering

### Phase 2: Grid Layout
- [ ] Create `ImageGrid.svelte` with responsive grid
- [ ] Create `ImageThumbnail.svelte` with larger thumbnails
- [ ] Create `ViewModeToggle.svelte` for grid/table switching
- [ ] Implement thumbnail size adjustment
- [ ] Test grid responsiveness

### Phase 3: Image Lightbox
- [ ] Create `ImageLightbox.svelte` modal component
- [ ] Add lightbox state to `view.svelte.ts`
- [ ] Implement keyboard navigation (arrow keys, escape, zoom)
- [ ] Add metadata overlay
- [ ] Test prev/next navigation

### Phase 4: Audio Player
- [ ] Create `AudioList.svelte` component
- [ ] Create `AudioPlayer.svelte` with HTML5 audio
- [ ] Implement play/pause/scrub controls
- [ ] Add volume control
- [ ] Handle single-player-at-a-time logic
- [ ] Test audio file loading with Tauri

### Phase 5: Filters
- [ ] Create `src/lib/state/filters.svelte.ts`
- [ ] Create `FilterPanel.svelte` component
- [ ] Update `queries.ts` with `searchAssetsWithFilters`
- [ ] Implement filter application and reset
- [ ] Test all filter types (format, size, dimensions, duration)

### Phase 6: UI Polish
- [ ] Create `Toolbar.svelte` consolidating controls
- [ ] Create `SortDropdown.svelte` component
- [ ] Add density CSS variables and classes
- [ ] Implement loading and empty states
- [ ] Add transition animations

### Phase 7: Integration
- [ ] Update `+page.svelte` with new layout
- [ ] Wire up all state management
- [ ] Test complete user flows
- [ ] Performance optimization (lazy loading, virtualization)
- [ ] Accessibility audit (keyboard nav, ARIA labels)

---

## Technical Considerations

### Performance Optimizations

1. **Thumbnail Loading**:
   - Implement lazy loading for grid thumbnails
   - Use `IntersectionObserver` to load only visible thumbnails
   - Cache loaded thumbnails in memory

2. **Virtual Scrolling** (Optional, Phase 8):
   - For large asset libraries (10,000+ items)
   - Use `svelte-virtual-list` or similar
   - Render only visible items + buffer

3. **Database Query Optimization**:
   - Add indexes for commonly filtered columns
   - Batch thumbnail loads (50-100 at a time)
   - Debounce search input (300ms delay)

4. **State Management**:
   - Avoid unnecessary re-renders with `$derived`
   - Use `$effect` cleanup for event listeners
   - Memoize expensive computations

### Accessibility

1. **Keyboard Navigation**:
   - Tab through grid items
   - Arrow keys for lightbox navigation
   - Space/Enter to activate buttons
   - Escape to close modals

2. **Screen Reader Support**:
   - Add ARIA labels to all interactive elements
   - Use semantic HTML (`<button>`, `<nav>`, `<main>`)
   - Announce state changes (loading, filtering applied)

3. **Focus Management**:
   - Trap focus in modals
   - Restore focus after modal close
   - Visible focus indicators

### Error Handling

1. **Failed Thumbnail Loads**:
   - Show placeholder icon
   - Retry mechanism (1-2 attempts)
   - Log errors for debugging

2. **Audio Playback Errors**:
   - Handle unsupported formats gracefully
   - Show error message to user
   - Fall back to download option

3. **Database Errors**:
   - Toast notifications for failures
   - Retry connection on timeout
   - Cache last successful query

### Browser Compatibility

1. **Tauri WebView**:
   - Test on Windows, macOS, Linux
   - Verify audio format support across platforms
   - Check CSS variable support (should be fine)

2. **File Path Handling**:
   - Use `convertFileSrc` for all file URLs
   - Handle Windows vs Unix path separators
   - Test ZIP entry paths

---

## Future Enhancements (Phase 8+)

### Advanced Features
1. **Bulk Operations**:
   - Multi-select with checkboxes
   - Batch export/delete
   - Tag management

2. **Collections/Albums**:
   - User-created asset groups
   - Drag and drop to organize
   - Export collections

3. **AI-Powered Search**:
   - Visual similarity search (similar images)
   - Content-based tagging
   - Duplicate detection

4. **Metadata Editing**:
   - Edit file metadata (EXIF, ID3)
   - Rename files
   - Add custom tags

5. **Advanced Audio Features**:
   - Waveform visualization
   - Playlist creation
   - Audio trimming/export

6. **Performance Dashboard**:
   - Storage usage by type
   - File format breakdown
   - Processing statistics

### UI Refinements
1. **Themes**:
   - Multiple color schemes
   - Custom accent colors
   - High contrast mode

2. **Layout Presets**:
   - Save custom view configurations
   - Quick layout switching
   - Per-folder preferences

3. **Comparison View**:
   - Side-by-side image comparison
   - Diff view for similar assets
   - A/B testing layouts

---

## Migration Notes

### Breaking Changes
- `AssetList.svelte` will be split into `ImageGrid.svelte` and `AudioList.svelte`
- `AssetThumbnail.svelte` replaced by `ImageThumbnail.svelte`
- State management significantly refactored
- Database queries updated with new filter parameters

### Backward Compatibility
- Existing database schema unchanged (no migration needed)
- All existing Tauri commands remain functional
- Original table view preserved as fallback option

### Testing Strategy
1. Unit tests for state management (Vitest)
2. Component tests for UI components
3. Integration tests for database queries
4. Manual E2E testing for user flows

---

## Success Metrics

### User Experience Goals
- **Faster Asset Discovery**: Users find assets in <5 seconds
- **Intuitive Navigation**: No tutorial needed for basic operations
- **Responsive UI**: All interactions feel <100ms latency
- **Error Recovery**: Clear error messages with actionable steps

### Technical Goals
- **Performance**: Grid renders 100+ thumbnails in <1 second
- **Stability**: Zero crashes during normal operation
- **Scalability**: Handles libraries of 50,000+ assets smoothly
- **Maintainability**: All components <300 lines, well-documented

---

## Appendix: Component Hierarchy

```
+page.svelte
├─ ScanControl
├─ TaskProgress
├─ TabBar
├─ Toolbar
│  ├─ Search Input
│  ├─ FilterPanel
│  ├─ SortDropdown
│  └─ ViewModeToggle
├─ Main Content (conditional)
│  ├─ ImageGrid (images + grid mode)
│  │  └─ ImageThumbnail (per item)
│  ├─ ImageTable (images + table mode)
│  │  └─ AssetTableRow (per item)
│  └─ AudioList (audio mode)
│     └─ AudioPlayer (per item)
└─ ImageLightbox (modal, conditional)
```

---

## Questions for Review

1. **Grid Columns**: Should thumbnail grid be fixed columns (4-6) or auto-fill responsive?
2. **Audio Player**: Inline per-row or single player at bottom (like Spotify)?
3. **Sorting**: Default sort by filename or date modified?
4. **Pagination**: Infinite scroll or traditional pagination?
5. **Thumbnail Size**: Store user preference in local storage or per-session?
6. **Filter Presets**: Should users be able to save filter combinations?
7. **Keyboard Shortcuts**: Which shortcuts are most important? (e.g., Ctrl+F for search)

---

**End of Implementation Plan**
