import type Database from '@tauri-apps/plugin-sql';
import type { Asset, FolderLocation, SearchExclude, SourceFolder } from '$lib/types';
import type { DurationFilter } from '$lib/state/assets.svelte';
import { invoke } from '@tauri-apps/api/core';

/** Common SELECT columns for asset queries (joins source_folders for folder_path) */
const ASSET_SELECT = `
	SELECT
		assets.id, assets.filename, assets.folder_id, assets.rel_path,
		assets.zip_file, assets.zip_entry,
		sf.path as folder_path,
		assets.asset_type, assets.format, assets.file_size,
		assets.created_at, assets.modified_at,
		image_metadata.width, image_metadata.height,
		audio_metadata.duration_ms, audio_metadata.sample_rate, audio_metadata.channels
	FROM assets
	JOIN source_folders sf ON assets.folder_id = sf.id
`;

const ASSET_JOINS = `
	LEFT JOIN image_metadata ON assets.id = image_metadata.asset_id
	LEFT JOIN audio_metadata ON assets.id = audio_metadata.asset_id
`;

/** Search column targeting type */
export type SearchColumn = 'anywhere' | 'filename' | 'path';

/**
 * Build FTS condition for dual-table search.
 * Short patterns (< 3 chars) use word table only with wildcard.
 * Longer patterns use UNION of both tables.
 */
function buildFtsCondition(
  searchText: string,
  searchColumn: SearchColumn,
  conditions: string[],
  params: unknown[],
): void {
  const trimmed = searchText.trim();
  if (!trimmed) return;

  // Column prefix for FTS5 column targeting
  const colPrefix =
    searchColumn === 'filename' ? 'filename:' : searchColumn === 'path' ? 'searchable_path:' : '';

  if (trimmed.length < 3) {
    // Short patterns: word table only with wildcard (trigram needs >= 3 chars)
    const wordQuery = `${colPrefix}${trimmed}*`;
    conditions.push(
      'assets.id IN (SELECT rowid FROM assets_fts_word WHERE assets_fts_word MATCH ?)',
    );
    params.push(wordQuery);
  } else {
    // Longer patterns: UNION both tables
    // Trigram: exact substring match (no wildcard needed)
    // Word: prefix match with wildcard
    const subQuery = `${colPrefix}${trimmed}`;
    const wordQuery = `${colPrefix}${trimmed}*`;
    conditions.push(
      `assets.id IN (
        SELECT rowid FROM assets_fts_sub WHERE assets_fts_sub MATCH ?
        UNION
        SELECT rowid FROM assets_fts_word WHERE assets_fts_word MATCH ?
      )`,
    );
    params.push(subQuery, wordQuery);
  }
}

/**
 * Build shared filter conditions for searchAssets and countSearchResults.
 * Returns conditions, params, and an audioJoin clause (needed by countSearchResults,
 * which doesn't include ASSET_JOINS automatically).
 */
function buildFilterConditions(
  searchText?: string,
  assetType?: string,
  durationFilter?: DurationFilter,
  folderLocation?: FolderLocation | null,
  searchColumn: SearchColumn = 'anywhere',
): { conditions: string[]; params: unknown[]; audioJoin: string } {
  const conditions: string[] = [];
  const params: unknown[] = [];

  if (searchText?.trim()) {
    buildFtsCondition(searchText, searchColumn, conditions, params);
  }

  if (assetType) {
    conditions.push('assets.asset_type = ?');
    params.push(assetType);
  }

  if (folderLocation) {
    addFolderFilterConditions(folderLocation, conditions, params);
  }

  const audioJoin = durationFilter
    ? 'LEFT JOIN audio_metadata ON assets.id = audio_metadata.asset_id'
    : '';

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

  return { conditions, params, audioJoin };
}

/**
 * Search for assets with optional full-text search and filtering
 */
export async function searchAssets(
  db: Database,
  searchText?: string,
  assetType?: string,
  limit: number = 50,
  offset: number = 0,
  durationFilter?: DurationFilter,
  folderLocation?: FolderLocation | null,
  searchColumn: SearchColumn = 'anywhere',
): Promise<Asset[]> {
  const { conditions, params } = buildFilterConditions(
    searchText,
    assetType,
    durationFilter,
    folderLocation,
    searchColumn,
  );
  const whereClause = conditions.length ? `WHERE ${conditions.join(' AND ')}` : '';

  const query = `
		${ASSET_SELECT}
		${ASSET_JOINS}
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
  durationFilter?: DurationFilter,
  folderLocation?: FolderLocation | null,
  searchColumn: SearchColumn = 'anywhere',
): Promise<number> {
  const { conditions, params, audioJoin } = buildFilterConditions(
    searchText,
    assetType,
    durationFilter,
    folderLocation,
    searchColumn,
  );
  const whereClause = conditions.length ? `WHERE ${conditions.join(' AND ')}` : '';

  const query = `SELECT COUNT(*) FROM assets ${audioJoin} ${whereClause}`;
  const result = await db.select<Array<{ 'COUNT(*)': number }>>(query, params);
  return result[0]['COUNT(*)'];
}

/**
 * Get total count of all assets
 */
export async function getAssetCount(db: Database): Promise<number> {
  const result = await db.select<Array<{ 'COUNT(*)': number }>>('SELECT COUNT(*) FROM assets');
  return result[0]['COUNT(*)'];
}

/**
 * Get count of assets by type
 */
export async function getAssetCountByType(
  db: Database,
  assetType: 'image' | 'audio',
): Promise<number> {
  const result = await db.select<Array<{ 'COUNT(*)': number }>>(
    'SELECT COUNT(*) FROM assets WHERE asset_type = ?',
    [assetType],
  );
  return result[0]['COUNT(*)'];
}

/**
 * Get counts of both image and audio assets
 */
export async function getAssetTypeCounts(db: Database): Promise<{ images: number; audio: number }> {
  const [images, audio] = await Promise.all([
    getAssetCountByType(db, 'image'),
    getAssetCountByType(db, 'audio'),
  ]);
  return { images, audio };
}

/**
 * Get thumbnail data for a specific asset
 */
export async function getThumbnail(
  db: Database,
  assetId: number,
): Promise<Uint8Array<ArrayBuffer> | null> {
  try {
    const result = await db.select<Array<{ thumbnail_data: number[] }>>(
      'SELECT thumbnail_data FROM image_metadata WHERE asset_id = ?',
      [assetId],
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
export async function getPendingAssetCounts(
  db: Database,
  preGenerateThumbnails = false,
): Promise<AssetPendingCounts> {
  // Count images without metadata, or missing thumbnail when thumbnails are enabled.
  // Exclude images ≤128px when checking thumbnails — they don't need thumbnails
  // (the original is already small enough to serve as its own thumbnail).
  const imagesResult = await db.select<Array<{ 'COUNT(*)': number }>>(
    preGenerateThumbnails
      ? `SELECT COUNT(*) FROM assets a
		 LEFT JOIN image_metadata im ON a.id = im.asset_id
		 WHERE a.asset_type = 'image' AND (im.asset_id IS NULL
		   OR (im.thumbnail_data IS NULL
		       AND NOT (im.width IS NOT NULL AND im.width <= 128
		               AND im.height IS NOT NULL AND im.height <= 128)))`
      : `SELECT COUNT(*) FROM assets a
		 LEFT JOIN image_metadata im ON a.id = im.asset_id
		 WHERE a.asset_type = 'image' AND im.asset_id IS NULL`,
  );

  // Count audio without metadata
  const audioResult = await db.select<Array<{ 'COUNT(*)': number }>>(
    `SELECT COUNT(*) FROM assets a
		 LEFT JOIN audio_metadata am ON a.id = am.asset_id
		 WHERE a.asset_type = 'audio' AND am.asset_id IS NULL`,
  );

  return {
    images: imagesResult[0]['COUNT(*)'],
    audio: audioResult[0]['COUNT(*)'],
  };
}

// ============================================================================
// Folder filter logic
// ============================================================================

/**
 * Build SQL conditions for a FolderLocation filter.
 */
function addFolderFilterConditions(loc: FolderLocation, conditions: string[], params: unknown[]) {
  conditions.push('assets.folder_id = ?');
  params.push(loc.folderId);

  if (loc.type === 'zip') {
    // ZIP-internal: exact rel_path + zip_file match, plus zip_entry prefix
    conditions.push('assets.rel_path = ?');
    params.push(loc.relPath);
    conditions.push('assets.zip_file = ?');
    params.push(loc.zipFile);
    if (loc.zipPrefix === '') {
      // ZIP root: all entries in this zip
      conditions.push('assets.zip_entry IS NOT NULL');
    } else {
      conditions.push('assets.zip_entry LIKE ?');
      params.push(loc.zipPrefix + '%');
    }
  } else {
    // Filesystem directory: match rel_path exactly or as prefix
    if (loc.relPath === '') {
      // Folder root: all assets in this folder
    } else {
      conditions.push('(assets.rel_path = ? OR assets.rel_path LIKE ?)');
      params.push(loc.relPath, loc.relPath + '/%');
    }
  }
}

// ============================================================================
// Directory browsing (Explore view)
// ============================================================================

export interface DirectoryNode {
  /** Unique key for this node in the tree */
  key: string;
  /** Row ID in the directories table (for parent_id lookups) */
  directoryId: number;
  name: string;
  childCount: number;
  assetCount: number;
  /** The FolderLocation this node represents (for filtering assets when selected) */
  location: FolderLocation;
}

/** Raw row from the directories table */
interface DirectoryRow {
  id: number;
  name: string;
  rel_path: string;
  zip_file: string | null;
  zip_prefix: string | null;
  asset_count: number;
  child_count: number;
  dir_type: string;
  folder_id: number;
}

/** Convert a directories table row to a DirectoryNode */
function rowToNode(row: DirectoryRow): DirectoryNode {
  const folderId = row.folder_id;
  if (row.dir_type === 'zipdir') {
    return {
      key: `zip:${folderId}:${row.rel_path}:${row.zip_file}:${row.zip_prefix}`,
      directoryId: row.id,
      name: row.name,
      childCount: row.child_count,
      assetCount: row.asset_count,
      location: {
        type: 'zip',
        folderId,
        relPath: row.rel_path,
        zipFile: row.zip_file!,
        zipPrefix: row.zip_prefix!,
      },
    };
  } else if (row.dir_type === 'zip') {
    return {
      key: `zip:${folderId}:${row.rel_path}:${row.zip_file}`,
      directoryId: row.id,
      name: row.name,
      childCount: row.child_count,
      assetCount: row.asset_count,
      location: {
        type: 'zip',
        folderId,
        relPath: row.rel_path,
        zipFile: row.zip_file!,
        zipPrefix: '',
      },
    };
  } else {
    return {
      key: `folder:${folderId}:${row.rel_path}`,
      directoryId: row.id,
      name: row.name,
      childCount: row.child_count,
      assetCount: row.asset_count,
      location: { type: 'folder', folderId, relPath: row.rel_path },
    };
  }
}

/**
 * Get source folders as root directory nodes.
 */
export async function getSourceFolderRoots(db: Database): Promise<DirectoryNode[]> {
  const folders = await db.select<SourceFolder[]>(
    `SELECT * FROM source_folders WHERE status = 'active' ORDER BY path COLLATE NOCASE`,
  );
  // Get root-level child counts from the directories table
  const rootCounts = await db.select<Array<{ folder_id: number; cnt: number }>>(
    `SELECT folder_id, COUNT(*) as cnt FROM directories
     WHERE parent_id IS NULL
     GROUP BY folder_id`,
  );
  const countMap = new Map(rootCounts.map((r) => [r.folder_id, r.cnt]));

  return folders.map((f) => ({
    key: `folder:${f.id}`,
    directoryId: 0, // root nodes don't have a directory row
    name: f.label || f.path.split(/[\\/]/).pop() || f.path,
    childCount: countMap.get(f.id) ?? 0,
    assetCount: f.asset_count,
    location: { type: 'folder' as const, folderId: f.id, relPath: '' },
  }));
}

/**
 * Get child directory nodes using the precomputed directories table.
 * For root-level children (directoryId === 0), queries by folder_id + parent_id IS NULL.
 */
export async function getDirectoryChildren(
  db: Database,
  directoryId: number,
  folderId: number,
): Promise<DirectoryNode[]> {
  let rows: DirectoryRow[];
  if (directoryId === 0) {
    // Root level: children with no parent
    rows = await db.select<DirectoryRow[]>(
      `SELECT id, name, rel_path, zip_file, zip_prefix, asset_count, child_count, dir_type, folder_id
       FROM directories
       WHERE folder_id = ? AND parent_id IS NULL
       ORDER BY dir_type, name COLLATE NOCASE`,
      [folderId],
    );
  } else {
    rows = await db.select<DirectoryRow[]>(
      `SELECT id, name, rel_path, zip_file, zip_prefix, asset_count, child_count, dir_type, folder_id
       FROM directories
       WHERE parent_id = ?
       ORDER BY dir_type, name COLLATE NOCASE`,
      [directoryId],
    );
  }
  return rows.map(rowToNode);
}


// ============================================================================
// Search excludes (per-folder/zip segment exclusion from search indexing)
// ============================================================================

/**
 * Get search excludes for a source folder
 */
export async function getSearchExcludes(db: Database, folderId: number): Promise<SearchExclude[]> {
  return db.select<SearchExclude[]>(
    `SELECT zip_file, excluded_path
     FROM folder_search_excludes
     WHERE source_folder_id = ?
     ORDER BY excluded_path COLLATE NOCASE`,
    [folderId],
  );
}

/**
 * Get all distinct rel_path values for a source folder (filesystem directories).
 * Used to build the search config tree. Reads from precomputed directories table.
 */
export async function getDistinctRelPaths(db: Database, folderId: number): Promise<string[]> {
  const rows = await db.select<Array<{ rel_path: string }>>(
    `SELECT rel_path FROM directories
     WHERE folder_id = ? AND dir_type = 'dir' AND rel_path != ''
     ORDER BY rel_path COLLATE NOCASE`,
    [folderId],
  );
  return rows.map((r) => r.rel_path);
}

/**
 * Get ZIP directory tree data for the search config panel.
 * Replaces the Rust `get_zip_dir_trees` command.
 */
export async function getZipDirTrees(
  db: Database,
  folderId: number,
): Promise<Array<{ rel_path: string; zip_file: string; dirs: string[] }>> {
  const rows = await db.select<Array<{ rel_path: string; zip_file: string; zip_prefix: string }>>(
    `SELECT rel_path, zip_file, zip_prefix FROM directories
     WHERE folder_id = ? AND dir_type = 'zipdir'
     ORDER BY rel_path, zip_file, zip_prefix`,
    [folderId],
  );

  // Group by (rel_path, zip_file) to match the old ZipDirGroup shape
  const groups = new Map<string, { rel_path: string; zip_file: string; dirs: string[] }>();
  for (const row of rows) {
    const key = `${row.rel_path}\0${row.zip_file}`;
    if (!groups.has(key)) {
      groups.set(key, { rel_path: row.rel_path, zip_file: row.zip_file, dirs: [] });
    }
    // Strip trailing slash from zip_prefix to match old format
    const dir = row.zip_prefix.endsWith('/') ? row.zip_prefix.slice(0, -1) : row.zip_prefix;
    groups.get(key)!.dirs.push(dir);
  }
  return [...groups.values()];
}


// ============================================================================
// CLAP Semantic Search (uses Tauri commands, not direct SQL)
// ============================================================================

/**
 * Result of a semantic search query - full Asset data plus similarity score.
 * The Rust backend always returns null for width/height (audio-only).
 */
export type SemanticSearchResult = Asset & { similarity: number };

/**
 * Convert a FolderLocation to the flat object the Rust FolderFilter expects.
 */
function toFolderFilter(loc: FolderLocation | null | undefined) {
  if (!loc) return null;
  return {
    folderId: loc.folderId,
    relPath: loc.relPath,
    zipFile: loc.type === 'zip' ? loc.zipFile : null,
    zipPrefix: loc.type === 'zip' ? loc.zipPrefix : null,
  };
}

/**
 * Search audio assets semantically using CLAP embeddings
 * Falls back to error if CLAP server is unavailable
 */
export async function searchAudioSemantic(
  query: string,
  limit: number = 50,
  durationFilter?: DurationFilter,
  folderLocation?: FolderLocation | null,
): Promise<SemanticSearchResult[]> {
  return invoke('search_audio_semantic', {
    query,
    limit,
    minDurationMs: durationFilter?.minMs ?? null,
    maxDurationMs: durationFilter?.maxMs ?? null,
    folderFilter: toFolderFilter(folderLocation),
  });
}

/**
 * Find audio assets similar to a given audio asset using its stored CLAP embedding
 */
export async function searchAudioBySimilarity(
  assetId: number,
  limit: number = 500,
  durationFilter?: DurationFilter,
  folderLocation?: FolderLocation | null,
): Promise<SemanticSearchResult[]> {
  return invoke('search_audio_by_similarity', {
    assetId,
    limit,
    minDurationMs: durationFilter?.minMs ?? null,
    maxDurationMs: durationFilter?.maxMs ?? null,
    folderFilter: toFolderFilter(folderLocation),
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

/**
 * CLAP server health info including device (CPU/GPU)
 */
export interface ClapServerInfo {
  status: string;
  model: string;
  device: string;
  embedding_dim: number;
  port: number;
}

/**
 * Get detailed CLAP server info (device, model, etc.)
 */
export async function getClapServerInfo(): Promise<ClapServerInfo> {
  return invoke('get_clap_server_info');
}

/**
 * Get the size of the uv/CLAP cache in bytes
 */
export async function getClapCacheSize(): Promise<number> {
  return invoke('get_clap_cache_size');
}

/**
 * Clear the uv/CLAP cache (Python, packages, uv binary)
 */
export async function clearClapCache(): Promise<void> {
  return invoke('clear_clap_cache');
}

/**
 * Check what CLAP setup artifacts exist on disk
 */
export interface ClapSetupStateInfo {
  uv_installed: boolean;
  cache_exists: boolean;
}

export async function checkClapSetupState(): Promise<ClapSetupStateInfo> {
  return invoke('check_clap_setup_state');
}
