import type Database from '@tauri-apps/plugin-sql';
import type { Asset, PendingCount } from '$lib/types';
import type { DurationFilter } from '$lib/state/assets.svelte';
import { invoke } from '@tauri-apps/api/core';

/**
 * Search for assets with optional full-text search and filtering
 */
export async function searchAssets(
	db: Database,
	searchText?: string,
	assetType?: string,
	limit: number = 50,
	offset: number = 0,
	durationFilter?: DurationFilter
): Promise<Asset[]> {
	const baseSelect = `
		SELECT
			assets.id, assets.filename, assets.path, assets.zip_entry, assets.asset_type,
			assets.format, assets.file_size, assets.created_at, assets.modified_at,
			image_metadata.width, image_metadata.height,
			audio_metadata.duration_ms, audio_metadata.sample_rate, audio_metadata.channels
		FROM assets
	`;

	const joins = `
		LEFT JOIN image_metadata ON assets.id = image_metadata.asset_id
		LEFT JOIN audio_metadata ON assets.id = audio_metadata.asset_id
	`;

	const ftsQuery = searchText?.trim() ? `${searchText.trim()}*` : null;
	const conditions: string[] = [];
	const params: unknown[] = [];

	// Use a subquery for FTS matching to force SQLite's query planner to
	// evaluate the FTS index first (see countSearchResults for details).
	if (ftsQuery) {
		conditions.push('assets.id IN (SELECT rowid FROM assets_fts WHERE assets_fts MATCH ?)');
		params.push(ftsQuery);
	}

	if (assetType) {
		conditions.push('assets.asset_type = ?');
		params.push(assetType);
	}

	// Duration filter (only applies to audio assets)
	if (durationFilter) {
		if (durationFilter.minMs !== null) {
			conditions.push('audio_metadata.duration_ms >= ?');
			params.push(durationFilter.minMs);
		}
		if (durationFilter.maxMs !== null) {
			conditions.push('audio_metadata.duration_ms <= ?');
			params.push(durationFilter.maxMs);
		}
	}

	const whereClause = conditions.length ? `WHERE ${conditions.join(' AND ')}` : '';

	const query = `
		${baseSelect}
		${joins}
		${whereClause}
		ORDER BY assets.filename COLLATE NOCASE ASC
		LIMIT ? OFFSET ?
	`;

	params.push(limit, offset);
	return db.select<Asset[]>(query, params);
}

/**
 * Count assets matching a search query (same filters as searchAssets, but returns count)
 */
export async function countSearchResults(
	db: Database,
	searchText?: string,
	assetType?: string,
	durationFilter?: DurationFilter
): Promise<number> {
	const ftsQuery = searchText?.trim() ? `${searchText.trim()}*` : null;
	const conditions: string[] = [];
	const params: unknown[] = [];

	// Use a subquery for FTS matching to force SQLite's query planner to
	// evaluate the FTS index first. A direct JOIN with additional WHERE
	// conditions (e.g. asset_type) can cause the planner to scan the assets
	// table first and probe FTS per-row, which effectively hangs on large datasets.
	if (ftsQuery) {
		conditions.push('assets.id IN (SELECT rowid FROM assets_fts WHERE assets_fts MATCH ?)');
		params.push(ftsQuery);
	}

	if (assetType) {
		conditions.push('assets.asset_type = ?');
		params.push(assetType);
	}

	const audioJoin = durationFilter ? 'LEFT JOIN audio_metadata ON assets.id = audio_metadata.asset_id' : '';

	if (durationFilter) {
		if (durationFilter.minMs !== null) {
			conditions.push('audio_metadata.duration_ms >= ?');
			params.push(durationFilter.minMs);
		}
		if (durationFilter.maxMs !== null) {
			conditions.push('audio_metadata.duration_ms <= ?');
			params.push(durationFilter.maxMs);
		}
	}

	const whereClause = conditions.length ? `WHERE ${conditions.join(' AND ')}` : '';

	const query = `SELECT COUNT(*) FROM assets ${audioJoin} ${whereClause}`;
	const result = await db.select<Array<{ 'COUNT(*)': number }>>(query, params);
	return result[0]['COUNT(*)'];
}

/**
 * Get total count of all assets
 */
export async function getAssetCount(db: Database): Promise<number> {
	const result = await db.select<Array<{ 'COUNT(*)': number }>>(
		'SELECT COUNT(*) FROM assets'
	);
	return result[0]['COUNT(*)'];
}

/**
 * Get count of assets by type
 */
export async function getAssetCountByType(
	db: Database,
	assetType: 'image' | 'audio'
): Promise<number> {
	const result = await db.select<Array<{ 'COUNT(*)': number }>>(
		'SELECT COUNT(*) FROM assets WHERE asset_type = ?',
		[assetType]
	);
	return result[0]['COUNT(*)'];
}

/**
 * Get counts of both image and audio assets
 */
export async function getAssetTypeCounts(
	db: Database
): Promise<{ images: number; audio: number }> {
	const [images, audio] = await Promise.all([
		getAssetCountByType(db, 'image'),
		getAssetCountByType(db, 'audio')
	]);
	return { images, audio };
}

/**
 * Get thumbnail data for a specific asset
 */
export async function getThumbnail(
	db: Database,
	assetId: number
): Promise<Uint8Array<ArrayBuffer> | null> {
	try {
		const result = await db.select<Array<{ thumbnail_data: number[] }>>(
			'SELECT thumbnail_data FROM image_metadata WHERE asset_id = ?',
			[assetId]
		);

		if (result.length === 0) {
			return null;
		}

		// Convert number array to Uint8Array with explicit ArrayBuffer
		const arr = new Uint8Array(result[0].thumbnail_data);
		// Ensure we have an ArrayBuffer-backed Uint8Array by creating a copy
		return new Uint8Array(arr.buffer.slice(0)) as Uint8Array<ArrayBuffer>;
	} catch (error) {
		console.error('Failed to get thumbnail:', error);
		return null;
	}
}

/**
 * Pending asset counts (images and audio only, clap is fetched separately)
 */
export interface AssetPendingCounts {
	images: number;
	audio: number;
}

/**
 * Get count of pending assets that need processing (images and audio metadata)
 */
export async function getPendingAssetCounts(db: Database): Promise<AssetPendingCounts> {
	// Count images without metadata
	const imagesResult = await db.select<Array<{ 'COUNT(*)': number }>>(
		`SELECT COUNT(*) FROM assets a
		 LEFT JOIN image_metadata im ON a.id = im.asset_id
		 WHERE a.asset_type = 'image' AND im.asset_id IS NULL`
	);

	// Count audio without metadata
	const audioResult = await db.select<Array<{ 'COUNT(*)': number }>>(
		`SELECT COUNT(*) FROM assets a
		 LEFT JOIN audio_metadata am ON a.id = am.asset_id
		 WHERE a.asset_type = 'audio' AND am.asset_id IS NULL`
	);

	return {
		images: imagesResult[0]['COUNT(*)'],
		audio: audioResult[0]['COUNT(*)']
	};
}

// ============================================================================
// Directory browsing (Explore view)
// ============================================================================

export interface DirectoryNode {
	path: string;
	name: string;
	childCount: number;
	assetCount: number;
}

/**
 * Get child directories of a given parent path.
 * If parentPath is null, returns root-level directories (scan roots).
 *
 * Uses the assets.path column to derive the directory hierarchy.
 * Paths are stored as the directory containing the asset file.
 */
export async function getDirectoryChildren(
	db: Database,
	parentPath: string | null
): Promise<DirectoryNode[]> {
	if (parentPath === null) {
		// Get root directories: distinct top-level paths
		// We find the shortest unique prefix directories
		const result = await db.select<Array<{ dir_path: string; asset_count: number }>>(
			`SELECT
				path as dir_path,
				COUNT(*) as asset_count
			FROM assets
			GROUP BY path
			ORDER BY path COLLATE NOCASE`
		);

		// Build a tree: find root prefixes
		return buildRootNodes(result);
	}

	// Get direct child directories of parentPath
	// Normalize: ensure consistent separator
	const normalizedParent = parentPath.replace(/\\/g, '/');
	const prefix = normalizedParent + '/';

	const result = await db.select<Array<{ dir_path: string; asset_count: number }>>(
		`SELECT
			path as dir_path,
			COUNT(*) as asset_count
		FROM assets
		WHERE REPLACE(path, '\\', '/') LIKE ? || '%'
		GROUP BY path
		ORDER BY path COLLATE NOCASE`,
		[prefix]
	);

	// Find immediate children: paths that have exactly one more segment after parent
	return buildChildNodes(normalizedParent, result);
}

function buildRootNodes(
	rows: Array<{ dir_path: string; asset_count: number }>
): DirectoryNode[] {
	// Group paths by their root prefix (e.g., "C:/Users/foo/assets")
	// Find the common shortest prefixes that contain assets
	const pathMap = new Map<string, { assetCount: number; childPaths: Set<string> }>();

	for (const row of rows) {
		const normalized = row.dir_path.replace(/\\/g, '/');
		// Find the scan root: we look for the shortest path that's a prefix of multiple asset paths
		// For simplicity, take the first 3 segments as root (e.g. C:/Users/folder or /home/user/folder)
		const segments = normalized.split('/');

		// Find a reasonable root: walk up from the full path until we find common prefixes
		// Simple heuristic: use the path itself if no children, otherwise find common prefix
		if (!pathMap.has(normalized)) {
			pathMap.set(normalized, { assetCount: 0, childPaths: new Set() });
		}
		pathMap.get(normalized)!.assetCount += row.asset_count;
	}

	// Now find the minimal set of root directories
	// A root is a path that is not a child of any other path in our set
	const allPaths = [...pathMap.keys()].sort();
	const roots = new Map<string, { assetCount: number; descendants: number }>();

	for (const path of allPaths) {
		// Find the shortest ancestor already in roots
		let foundRoot = false;
		for (const [rootPath] of roots) {
			if (path.startsWith(rootPath + '/')) {
				// This path is under an existing root
				roots.get(rootPath)!.descendants++;
				roots.get(rootPath)!.assetCount += pathMap.get(path)!.assetCount;
				foundRoot = true;
				break;
			}
		}
		if (!foundRoot) {
			// Check if this path should absorb any existing roots
			const absorbed: string[] = [];
			let totalAssets = pathMap.get(path)!.assetCount;
			let totalDescendants = 0;
			for (const [rootPath, rootData] of roots) {
				if (rootPath.startsWith(path + '/')) {
					absorbed.push(rootPath);
					totalAssets += rootData.assetCount;
					totalDescendants += rootData.descendants + 1;
				}
			}
			for (const a of absorbed) roots.delete(a);
			roots.set(path, { assetCount: totalAssets, descendants: totalDescendants });
		}
	}

	return [...roots.entries()].map(([path, data]) => {
		const segments = path.split('/');
		return {
			path,
			name: segments[segments.length - 1] || segments[segments.length - 2] || path,
			childCount: data.descendants,
			assetCount: data.assetCount,
		};
	});
}

function buildChildNodes(
	parentPath: string,
	rows: Array<{ dir_path: string; asset_count: number }>
): DirectoryNode[] {
	// Group by immediate child segment
	const childMap = new Map<string, { fullPath: string; assetCount: number; childPaths: number }>();
	const parentDepth = parentPath.split('/').length;

	for (const row of rows) {
		const normalized = row.dir_path.replace(/\\/g, '/');
		if (!normalized.startsWith(parentPath + '/')) continue;

		const segments = normalized.split('/');
		// Immediate child is at parentDepth index
		if (segments.length <= parentDepth) continue;

		const childSegment = segments[parentDepth];
		const childPath = segments.slice(0, parentDepth + 1).join('/');

		if (!childMap.has(childPath)) {
			childMap.set(childPath, { fullPath: childPath, assetCount: 0, childPaths: 0 });
		}
		const entry = childMap.get(childPath)!;
		entry.assetCount += row.asset_count;
		// If the normalized path is longer than immediate child, it's a deeper descendant
		if (segments.length > parentDepth + 1) {
			entry.childPaths++;
		}
	}

	return [...childMap.entries()].map(([path, data]) => {
		const segments = path.split('/');
		return {
			path,
			name: segments[segments.length - 1],
			childCount: data.childPaths,
			assetCount: data.assetCount,
		};
	});
}

/**
 * Get assets in a specific directory (exact path match, not recursive)
 */
export async function getAssetsInDirectory(
	db: Database,
	directoryPath: string
): Promise<Asset[]> {
	const normalized = directoryPath.replace(/\\/g, '/');
	return db.select<Asset[]>(
		`SELECT
			assets.id, assets.filename, assets.path, assets.zip_entry, assets.asset_type,
			assets.format, assets.file_size, assets.created_at, assets.modified_at,
			image_metadata.width, image_metadata.height,
			audio_metadata.duration_ms, audio_metadata.sample_rate, audio_metadata.channels
		FROM assets
		LEFT JOIN image_metadata ON assets.id = image_metadata.asset_id
		LEFT JOIN audio_metadata ON assets.id = audio_metadata.asset_id
		WHERE REPLACE(assets.path, '\\', '/') = ?
		ORDER BY assets.filename COLLATE NOCASE ASC`,
		[normalized]
	);
}

// ============================================================================
// CLAP Semantic Search (uses Tauri commands, not direct SQL)
// ============================================================================

/**
 * Result of a semantic search query - includes full asset data for direct use
 */
export interface SemanticSearchResult {
	// Asset fields
	id: number;
	filename: string;
	path: string;
	zip_entry: string | null;
	asset_type: string;
	format: string;
	file_size: number;
	created_at: number;
	modified_at: number;
	// Audio metadata (nullable)
	duration_ms: number | null;
	sample_rate: number | null;
	channels: number | null;
	// Similarity score
	similarity: number;
}

/**
 * Search audio assets semantically using CLAP embeddings
 * Falls back to error if CLAP server is unavailable
 */
export async function searchAudioSemantic(
	query: string,
	limit: number = 50,
	durationFilter?: DurationFilter
): Promise<SemanticSearchResult[]> {
	return invoke('search_audio_semantic', {
		query,
		limit,
		minDurationMs: durationFilter?.minMs ?? null,
		maxDurationMs: durationFilter?.maxMs ?? null
	});
}

/**
 * Get count of audio assets pending CLAP embedding
 */
export async function getPendingClapCount(): Promise<number> {
	return invoke('get_pending_clap_count');
}

/**
 * Check if CLAP server is available
 */
export async function checkClapServer(): Promise<boolean> {
	return invoke('check_clap_server');
}

/**
 * Start the CLAP server if not running
 */
export async function startClapServer(): Promise<void> {
	return invoke('start_clap_server');
}
