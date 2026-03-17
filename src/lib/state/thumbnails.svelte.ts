import { SvelteMap } from 'svelte/reactivity';
import { invoke } from '@tauri-apps/api/core';
import { getDatabase } from '$lib/database/connection';
import { getThumbnail } from '$lib/database/queries';

/** Cached blob URLs for thumbnails, keyed by asset ID */
const cache = new SvelteMap<number, string>();

/** Asset IDs that have been requested but not yet fetched */
const pending = new Set<number>();

/** Asset IDs that failed or have no thumbnail available */
const failed = new Set<number>();

let batchTimer: ReturnType<typeof setTimeout> | null = null;

/** Batch debounce delay in ms */
const BATCH_DELAY = 50;

/** Observable thumbnail loading metrics */
class ThumbnailMetrics {
	/** Number of thumbnails waiting to be processed */
	queued = $state(0);
	/** Number currently being generated/loaded in a batch */
	inflight = $state(0);
	/** Total loaded since last cache clear */
	loaded = $state(0);
	/** Total failed since last cache clear */
	failedCount = $state(0);
	/** Thumbnails loaded per second (rolling average) */
	rate = $state(0);

	private recentTimestamps: number[] = [];

	recordLoaded(count: number) {
		this.loaded += count;
		const now = Date.now();
		for (let i = 0; i < count; i++) this.recentTimestamps.push(now);
		this.updateRate(now);
	}

	recordFailed(count: number) {
		this.failedCount += count;
	}

	private updateRate(now: number) {
		// Keep last 5 seconds of timestamps
		const cutoff = now - 5000;
		this.recentTimestamps = this.recentTimestamps.filter(t => t > cutoff);
		this.rate = Math.round(this.recentTimestamps.length / 5);
	}

	reset() {
		this.queued = 0;
		this.inflight = 0;
		this.loaded = 0;
		this.failedCount = 0;
		this.rate = 0;
		this.recentTimestamps = [];
	}
}

export const thumbnailMetrics = new ThumbnailMetrics();

/**
 * Get the cached thumbnail URL for an asset, or null if not yet available.
 * Pure read from cache — use requestThumbnail() to trigger loading.
 */
export function getThumbnailUrl(assetId: number): string | null {
	return cache.get(assetId) ?? null;
}

/**
 * Check if a thumbnail request has failed (no thumbnail available).
 */
export function hasThumbnailFailed(assetId: number): boolean {
	return failed.has(assetId);
}

/**
 * Request that a thumbnail be loaded for the given asset ID.
 * The request is batched and debounced. When ready, the URL
 * will appear in the cache (readable via getThumbnailUrl).
 */
export function requestThumbnail(assetId: number): void {
	if (cache.has(assetId) || pending.has(assetId) || failed.has(assetId)) return;
	pending.add(assetId);
	thumbnailMetrics.queued = pending.size;
	scheduleBatch();
}

/**
 * Cancel a pending thumbnail request. Call when a component unmounts
 * (e.g., virtual scroll removes an item that scrolled out of view).
 * If the item scrolls back into view and remounts, it will re-request.
 */
export function cancelThumbnail(assetId: number): void {
	pending.delete(assetId);
	thumbnailMetrics.queued = pending.size;
}

/** Max IDs to send per IPC call — keeps memory bounded on the backend */
const MAX_BATCH_SIZE = 10;

let processing = false;

function scheduleBatch() {
	if (batchTimer !== null) return;
	batchTimer = setTimeout(processBatch, BATCH_DELAY);
}

async function processBatch() {
	batchTimer = null;
	if (processing) return; // prevent overlapping batches
	processing = true;

	try {
		while (pending.size > 0) {
			// Take a small batch from pending (most recently added = currently visible)
			const allPending = [...pending];
			const batch = allPending.slice(-MAX_BATCH_SIZE);
			for (const id of batch) pending.delete(id);
			thumbnailMetrics.queued = pending.size;
			thumbnailMetrics.inflight = batch.length;

			try {
				// Ask backend to generate any missing thumbnails
				await invoke('ensure_thumbnails', { assetIds: batch });

				// Read thumbnails from DB
				const db = await getDatabase();
				let batchLoaded = 0;
				let batchFailed = 0;
				for (const id of batch) {
					const data = await getThumbnail(db, id);
					if (data) {
						const blob = new Blob([data], { type: 'image/webp' });
						cache.set(id, URL.createObjectURL(blob));
						batchLoaded++;
					} else {
						failed.add(id);
						batchFailed++;
					}
				}
				thumbnailMetrics.recordLoaded(batchLoaded);
				thumbnailMetrics.recordFailed(batchFailed);
			} catch (e) {
				console.error('Failed to load thumbnails:', e);
				for (const id of batch) {
					if (!cache.has(id)) {
						failed.add(id);
					}
				}
			}

			thumbnailMetrics.inflight = 0;
		}
	} finally {
		processing = false;
		thumbnailMetrics.queued = 0;
		thumbnailMetrics.inflight = 0;
	}
}

/**
 * Clear all cached thumbnails. Call when the asset list changes
 * (e.g., new search, folder change) to free memory.
 */
export function clearThumbnailCache(): void {
	for (const url of cache.values()) {
		URL.revokeObjectURL(url);
	}
	cache.clear();
	failed.clear();
	pending.clear();
	if (batchTimer !== null) {
		clearTimeout(batchTimer);
		batchTimer = null;
	}
	thumbnailMetrics.reset();
}
