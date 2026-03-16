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
	scheduleBatch();
}

function scheduleBatch() {
	if (batchTimer !== null) return;
	batchTimer = setTimeout(processBatch, BATCH_DELAY);
}

async function processBatch() {
	batchTimer = null;
	const ids = [...pending];
	pending.clear();
	if (ids.length === 0) return;

	try {
		// Ask backend to generate any missing thumbnails
		await invoke('ensure_thumbnails', { assetIds: ids });

		// Read thumbnails from DB
		const db = await getDatabase();
		for (const id of ids) {
			const data = await getThumbnail(db, id);
			if (data) {
				const blob = new Blob([data], { type: 'image/webp' });
				cache.set(id, URL.createObjectURL(blob));
			} else {
				failed.add(id);
			}
		}
	} catch (e) {
		console.error('Failed to load thumbnails:', e);
		// Mark all as failed so we don't retry endlessly
		for (const id of ids) {
			if (!cache.has(id)) {
				failed.add(id);
			}
		}
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
}
