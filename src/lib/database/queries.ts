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
  durationFilter?: DurationFilter,
  folderPath?: string | null,
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

  // Folder path filter (filesystem or ZIP-internal)
  if (folderPath) {
    addFolderFilterConditions(folderPath, conditions, params);
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
  durationFilter?: DurationFilter,
  folderPath?: string | null,
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

  // Folder path filter (filesystem or ZIP-internal)
  if (folderPath) {
    addFolderFilterConditions(folderPath, conditions, params);
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
export async function getPendingAssetCounts(db: Database): Promise<AssetPendingCounts> {
  // Count images without metadata
  const imagesResult = await db.select<Array<{ 'COUNT(*)': number }>>(
    `SELECT COUNT(*) FROM assets a
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
// Directory browsing (Explore view)
// ============================================================================

export interface DirectoryNode {
  path: string;
  name: string;
  childCount: number;
  assetCount: number;
  /** If set, this is a ZIP-internal node. Value is the prefix within the zip ('' for zip root). */
  zipPrefix?: string;
}

/** Separator used to encode ZIP-internal paths: "C:\path\to\archive.zip::subfolder/" */
export const ZIP_SEP = '::';

/** Split a path on both \ and / separators */
function splitPath(p: string): string[] {
  return p.split(/[\\/]/);
}

/** Detect the separator used in a DB path */
function pathSep(p: string): string {
  return p.includes('\\') ? '\\' : '/';
}

/**
 * Parse a folder filter string into filesystem path + optional zip prefix.
 * "C:\path\archive.zip::images/" → { fsPath: "C:\path\archive.zip", zipPrefix: "images/" }
 * "C:\path\dir" → { fsPath: "C:\path\dir", zipPrefix: undefined }
 */
export function parseFolderFilter(folderPath: string): { fsPath: string; zipPrefix?: string } {
  const sepIdx = folderPath.indexOf(ZIP_SEP);
  if (sepIdx === -1) return { fsPath: folderPath };
  return {
    fsPath: folderPath.substring(0, sepIdx),
    zipPrefix: folderPath.substring(sepIdx + ZIP_SEP.length),
  };
}

/**
 * Build folder filter SQL conditions for a given folderPath.
 * Handles both filesystem directories and ZIP-internal paths.
 */
function addFolderFilterConditions(folderPath: string, conditions: string[], params: unknown[]) {
  const { fsPath, zipPrefix } = parseFolderFilter(folderPath);
  if (zipPrefix !== undefined) {
    // ZIP-internal: exact path match + zip_entry prefix
    conditions.push('assets.path = ?');
    params.push(fsPath);
    if (zipPrefix === '') {
      // ZIP root: all entries in this zip
      conditions.push('assets.zip_entry IS NOT NULL');
    } else {
      conditions.push('assets.zip_entry LIKE ?');
      params.push(zipPrefix + '%');
    }
  } else {
    // Filesystem directory: recursive path match
    const sep = pathSep(fsPath);
    conditions.push('(assets.path = ? OR assets.path LIKE ?)');
    params.push(fsPath, fsPath + sep + '%');
  }
}

/**
 * Get child directories of a given parent path.
 * If parentPath is null, returns root-level directories (scan roots).
 *
 * Paths are stored in the DB with native OS separators and queried directly
 * against idx_assets_path for fast index lookups.
 */
/**
 * Get root directories quickly (without asset counts).
 * Returns roots with assetCount=0 and childCount=1 (assumed expandable).
 */
export async function getDirectoryRoots(db: Database): Promise<DirectoryNode[]> {
  const roots = await db.select<Array<{ root_path: string }>>(
    `SELECT DISTINCT root_path FROM scan_sessions ORDER BY root_path COLLATE NOCASE`,
  );
  return roots.map(({ root_path }) => {
    const segments = splitPath(root_path);
    return {
      path: root_path,
      name: segments[segments.length - 1] || root_path,
      childCount: 1, // assume expandable until counts loaded
      assetCount: 0,
    };
  });
}

/**
 * Load asset counts and child counts for root directories.
 * Filters out roots with no assets.
 */
export async function getDirectoryRootCounts(
  db: Database,
  roots: DirectoryNode[],
): Promise<DirectoryNode[]> {
  const results: DirectoryNode[] = [];
  for (const root of roots) {
    const sep = pathSep(root.path);
    const countResult = await db.select<Array<{ asset_count: number }>>(
      `SELECT COUNT(*) as asset_count
      FROM assets
      WHERE path LIKE ?`,
      [root.path + sep + '%'],
    );

    if (countResult[0].asset_count > 0) {
      results.push({
        ...root,
        childCount: 1, // roots are always expandable if they have assets
        assetCount: countResult[0].asset_count,
      });
    }
  }
  return results;
}

export async function getDirectoryChildren(
  db: Database,
  parentPath: string | null,
): Promise<DirectoryNode[]> {
  if (parentPath === null) {
    // Use the combined root loading functions
    const db2 = db;
    const roots = await getDirectoryRoots(db2);
    return getDirectoryRootCounts(db2, roots);
  }

  // Get child directories of parentPath
  // Query with LIKE prefix on the raw path — uses idx_assets_path
  const sep = pathSep(parentPath);
  const [result, zipResult] = await Promise.all([
    db.select<Array<{ dir_path: string; asset_count: number }>>(
      `SELECT path as dir_path, COUNT(*) as asset_count
			FROM assets
			WHERE path LIKE ?
			GROUP BY path
			ORDER BY path COLLATE NOCASE`,
      [parentPath + sep + '%'],
    ),
    // Find paths that are ZIP files (have zip_entry values) — these become expandable nodes
    db.select<Array<{ path: string }>>(
      `SELECT DISTINCT path FROM assets
			WHERE path LIKE ? AND zip_entry IS NOT NULL`,
      [parentPath + sep + '%'],
    ),
  ]);

  const zipPaths = new Set(zipResult.map((r) => r.path));
  return buildChildNodes(parentPath, result, zipPaths);
}

function buildChildNodes(
  parentPath: string,
  rows: Array<{ dir_path: string; asset_count: number }>,
  /** Paths that are ZIP files (have zip_entry assets) — they always get childCount > 0 */
  zipPaths?: Set<string>,
): DirectoryNode[] {
  const parentDepth = splitPath(parentPath).length;
  const sep = pathSep(parentPath);
  const childMap = new Map<
    string,
    { assetCount: number; subDirs: Set<string>; isZip: boolean }
  >();

  for (const row of rows) {
    const segments = splitPath(row.dir_path);
    if (segments.length <= parentDepth) continue;

    // path column stores full file paths (including filename).
    // Entries at exactly parentDepth+1 are individual files in the parent — skip them,
    // UNLESS they are ZIP files (which are browsable and should appear as tree nodes).
    if (segments.length === parentDepth + 1) {
      if (!zipPaths?.has(row.dir_path)) continue;
      // ZIP file: create a node for it (will be marked as zip below)
      const childPath = row.dir_path;
      if (!childMap.has(childPath)) {
        childMap.set(childPath, { assetCount: 0, subDirs: new Set(), isZip: false });
      }
      childMap.get(childPath)!.assetCount += row.asset_count;
      continue;
    }

    // Reconstruct the immediate child directory path
    const childPath = segments.slice(0, parentDepth + 1).join(sep);

    if (!childMap.has(childPath)) {
      childMap.set(childPath, { assetCount: 0, subDirs: new Set(), isZip: false });
    }
    const entry = childMap.get(childPath)!;
    entry.assetCount += row.asset_count;

    // Track actual subdirectories (entries with more than just a filename below the child)
    if (segments.length > parentDepth + 2) {
      entry.subDirs.add(segments[parentDepth + 1]);
    }
  }

  // Mark ZIP file paths as having children (browsable)
  if (zipPaths) {
    for (const zp of zipPaths) {
      const segments = splitPath(zp);
      if (segments.length !== parentDepth + 1) continue;
      const childPath = segments.slice(0, parentDepth + 1).join(sep);
      const entry = childMap.get(childPath);
      if (entry) {
        entry.isZip = true;
      }
    }
  }

  return [...childMap.entries()].map(([path, data]) => {
    const segments = splitPath(path);
    return {
      path,
      name: segments[segments.length - 1],
      childCount: data.isZip ? Math.max(1, data.subDirs.size) : data.subDirs.size,
      assetCount: data.assetCount,
      zipPrefix: data.isZip ? '' : undefined,
    };
  });
}

/**
 * Get child directories inside a ZIP file.
 * zipPath: the filesystem path to the .zip file
 * prefix: the directory prefix within the zip ('' for root, 'subfolder/' for a subfolder)
 */
export async function getZipDirectoryChildren(
  db: Database,
  zipPath: string,
  prefix: string,
): Promise<DirectoryNode[]> {
  // Get all zip_entry values under this prefix
  const likePattern = prefix === '' ? '%' : prefix + '%';
  const rows = await db.select<Array<{ zip_entry: string }>>(
    `SELECT zip_entry FROM assets
		WHERE path = ? AND zip_entry IS NOT NULL AND zip_entry LIKE ?`,
    [zipPath, likePattern],
  );

  // Build directory structure from zip_entry paths
  // zip_entry uses forward slashes (e.g., "images/textures/stone.jpg")
  const prefixDepth = prefix === '' ? 0 : prefix.split('/').filter(Boolean).length;
  const childMap = new Map<
    string,
    { assetCount: number; subDirs: Set<string>; hasDirectFiles: boolean; isNestedZip: boolean }
  >();

  for (const row of rows) {
    const entryPath = row.zip_entry;
    const segments = entryPath.split('/').filter(Boolean);
    if (segments.length <= prefixDepth) continue;

    // The immediate child segment
    const childName = segments[prefixDepth];

    if (segments.length === prefixDepth + 1) {
      // This is a direct file at the current prefix level — not a directory node
      // (e.g., a file sitting directly in the browsed directory)
      // Check if it's a nested ZIP file
      if (childName.toLowerCase().endsWith('.zip')) {
        if (!childMap.has(childName)) {
          childMap.set(childName, {
            assetCount: 0,
            subDirs: new Set(),
            hasDirectFiles: false,
            isNestedZip: true,
          });
        }
        childMap.get(childName)!.isNestedZip = true;
        childMap.get(childName)!.assetCount++;
      }
      // Regular files at this level are not shown as tree nodes
      continue;
    }

    // This entry is inside a child directory
    if (!childMap.has(childName)) {
      childMap.set(childName, {
        assetCount: 0,
        subDirs: new Set(),
        hasDirectFiles: false,
        isNestedZip: false,
      });
    }
    const entry = childMap.get(childName)!;
    entry.assetCount++;

    if (segments.length === prefixDepth + 2) {
      // Direct file in the child directory (e.g., "childDir/file.wav")
      entry.hasDirectFiles = true;
    } else {
      // File in a subdirectory of the child (e.g., "childDir/subDir/file.wav")
      entry.subDirs.add(segments[prefixDepth + 1]);
    }
  }

  return [...childMap.entries()]
    .filter(([_, data]) => data.subDirs.size > 0 || data.hasDirectFiles || data.isNestedZip)
    .map(([name, data]) => {
      const nodePrefix = prefix + name + '/';
      return {
        path: zipPath + ZIP_SEP + nodePrefix,
        name,
        childCount: data.subDirs.size,
        assetCount: data.assetCount,
        zipPrefix: nodePrefix,
      };
    })
    .sort((a, b) => a.name.localeCompare(b.name));
}

/**
 * Get assets in a specific directory (exact path match, not recursive)
 */
export async function getAssetsInDirectory(db: Database, directoryPath: string): Promise<Asset[]> {
  return db.select<Asset[]>(
    `SELECT
			assets.id, assets.filename, assets.path, assets.zip_entry, assets.asset_type,
			assets.format, assets.file_size, assets.created_at, assets.modified_at,
			image_metadata.width, image_metadata.height,
			audio_metadata.duration_ms, audio_metadata.sample_rate, audio_metadata.channels
		FROM assets
		LEFT JOIN image_metadata ON assets.id = image_metadata.asset_id
		LEFT JOIN audio_metadata ON assets.id = audio_metadata.asset_id
		WHERE assets.path = ?
		ORDER BY assets.filename COLLATE NOCASE ASC`,
    [directoryPath],
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
  durationFilter?: DurationFilter,
): Promise<SemanticSearchResult[]> {
  return invoke('search_audio_semantic', {
    query,
    limit,
    minDurationMs: durationFilter?.minMs ?? null,
    maxDurationMs: durationFilter?.maxMs ?? null,
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
