import { SvelteMap } from 'svelte/reactivity';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getDatabase } from '$lib/database/connection';
import { getThumbnail } from '$lib/database/queries';

// ---------------------------------------------------------------------------
// Cache & state
// ---------------------------------------------------------------------------

/** Cached blob URLs for thumbnails, keyed by asset ID */
const cache = new SvelteMap<number, string>();

/** Asset IDs that failed or have no thumbnail available */
const failed = new Set<number>();

/** IDs we've already sent to the backend (dedup) */
const requested = new Set<number>();

/** Incremented each time the cache is fully cleared — lets components react and re-request */
export const cacheReset = $state({ version: 0 });

// ---------------------------------------------------------------------------
// Stats (updated by backend events)
// ---------------------------------------------------------------------------

class ThumbnailMetrics {
  queued = $state(0);
  processing = $state(0);
  loaded = $state(0);
  failedCount = $state(0);
  rate = $state(0);

  reset() {
    this.queued = 0;
    this.processing = 0;
    this.loaded = 0;
    this.failedCount = 0;
    this.rate = 0;
  }
}

export const thumbnailMetrics = new ThumbnailMetrics();

// ---------------------------------------------------------------------------
// Event listeners (initialized once)
// ---------------------------------------------------------------------------

let listenersReady = false;

async function ensureListeners() {
  if (listenersReady) return;
  listenersReady = true;

  // Listen for individual thumbnail completions
  await listen<{ asset_id: number; success: boolean }>('thumbnail-ready', async (event) => {
    const { asset_id, success } = event.payload;
    // Keep in `requested` until cache/failed is populated to prevent re-requests
    // during the async DB read window (reactive cascades from asset.width/height
    // patches could re-trigger effects before cache.set completes).

    if (!success) {
      failed.add(asset_id);
      requested.delete(asset_id);
      return;
    }

    // Already cached (e.g. from a previous generation)?
    if (cache.has(asset_id)) {
      requested.delete(asset_id);
      return;
    }

    // Read the thumbnail blob from DB
    try {
      const db = await getDatabase();
      const data = await getThumbnail(db, asset_id);
      if (data) {
        const blob = new Blob([data], { type: 'image/webp' });
        cache.set(asset_id, URL.createObjectURL(blob));
      } else {
        failed.add(asset_id);
      }
    } catch {
      failed.add(asset_id);
    }
    requested.delete(asset_id);
  });

  // Listen for periodic stats from the backend worker
  await listen<{
    queued: number;
    processing: number;
    loaded: number;
    failed: number;
    rate: number;
  }>('thumbnail-stats', (event) => {
    const s = event.payload;
    thumbnailMetrics.queued = s.queued;
    thumbnailMetrics.processing = s.processing;
    thumbnailMetrics.loaded = s.loaded;
    thumbnailMetrics.failedCount = s.failed;
    thumbnailMetrics.rate = Math.round(s.rate * 10) / 10;
  });
}

// Start listeners immediately on import
ensureListeners();

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Get the cached thumbnail URL for an asset, or null if not yet available.
 */
export function getThumbnailUrl(assetId: number): string | null {
  return cache.get(assetId) ?? null;
}

/**
 * Check if a thumbnail request has failed.
 */
export function hasThumbnailFailed(assetId: number): boolean {
  return failed.has(assetId);
}

/** Batch buffer + debounce for requests */
let requestBuffer: number[] = [];
let requestTimer: ReturnType<typeof setTimeout> | null = null;
const REQUEST_DELAY = 30; // ms

/** Batch buffer + debounce for cancels */
let cancelBuffer: number[] = [];
let cancelTimer: ReturnType<typeof setTimeout> | null = null;
const CANCEL_DELAY = 30; // ms

/**
 * Request that a thumbnail be loaded for the given asset ID.
 * Batched and sent to the backend worker via IPC.
 */
export function requestThumbnail(assetId: number): void {
  if (cache.has(assetId) || failed.has(assetId) || requested.has(assetId)) return;
  requested.add(assetId);
  requestBuffer.push(assetId);
  if (!requestTimer) {
    requestTimer = setTimeout(flushRequests, REQUEST_DELAY);
  }
}

function flushRequests() {
  requestTimer = null;
  if (requestBuffer.length === 0) return;
  const ids = requestBuffer;
  requestBuffer = [];
  invoke('request_thumbnails', { assetIds: ids }).catch((e: unknown) => {
    console.error('Failed to request thumbnails:', e);
  });
}

/**
 * Cancel a pending thumbnail request (component unmounted / scrolled away).
 */
export function cancelThumbnail(assetId: number): void {
  if (!requested.has(assetId)) return;
  requested.delete(assetId);
  cancelBuffer.push(assetId);
  if (!cancelTimer) {
    cancelTimer = setTimeout(flushCancels, CANCEL_DELAY);
  }
}

function flushCancels() {
  cancelTimer = null;
  if (cancelBuffer.length === 0) return;
  const ids = cancelBuffer;
  cancelBuffer = [];
  invoke('cancel_thumbnails', { assetIds: ids }).catch((e: unknown) => {
    console.error('Failed to cancel thumbnails:', e);
  });
}

/**
 * Clear all cached thumbnails. Call when the asset list changes.
 * Also clears the backend worker queue so stale requests don't accumulate.
 */
export function clearThumbnailCache(): void {
  for (const url of cache.values()) {
    URL.revokeObjectURL(url);
  }
  cache.clear();
  failed.clear();
  requested.clear();
  requestBuffer = [];
  cancelBuffer = [];
  if (requestTimer) {
    clearTimeout(requestTimer);
    requestTimer = null;
  }
  if (cancelTimer) {
    clearTimeout(cancelTimer);
    cancelTimer = null;
  }
  thumbnailMetrics.reset();
  cacheReset.version++;
  // Tell backend worker to drop its pending queue too — without this,
  // old requests stay queued and inflate the "Loading thumbnails" count.
  invoke('clear_thumbnail_queue').catch((e: unknown) => {
    console.error('Failed to clear thumbnail queue:', e);
  });
}
