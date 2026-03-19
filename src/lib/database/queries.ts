import type Database from '@tauri-apps/plugin-sql';
import type { Asset, FolderLocation, SourceFolder } from '$lib/types';
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
  const colPrefix = searchColumn === 'filename' ? 'filename:' : searchColumn === 'path' ? 'searchable_path:' : '';

  if (trimmed.length < 3) {
    // Short patterns: word table only with wildcard (trigram needs >= 3 chars)
    const wordQuery = `${colPrefix}${trimmed}*`;
    conditions.push('assets.id IN (SELECT rowid FROM assets_fts_word WHERE assets_fts_word MATCH ?)');
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
  const conditions: string[] = [];
  const params: unknown[] = [];

  // Use dual FTS tables for search
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
  const conditions: string[] = [];
  const params: unknown[] = [];

  // Use dual FTS tables for search
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
// Folder filter logic
// ============================================================================

/**
 * Build SQL conditions for a FolderLocation filter.
 */
function addFolderFilterConditions(
  loc: FolderLocation,
  conditions: string[],
  params: unknown[],
) {
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
      conditions.push("(assets.rel_path = ? OR assets.rel_path LIKE ?)");
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
  name: string;
  childCount: number;
  assetCount: number;
  /** The FolderLocation this node represents (for filtering assets when selected) */
  location: FolderLocation;
}

/**
 * Get source folders as root directory nodes.
 */
export async function getSourceFolderRoots(db: Database): Promise<DirectoryNode[]> {
  const folders = await db.select<SourceFolder[]>(
    `SELECT * FROM source_folders WHERE status = 'active' ORDER BY path COLLATE NOCASE`,
  );
  return folders.map((f) => ({
    key: `folder:${f.id}`,
    name: f.label || f.path.split(/[\\/]/).pop() || f.path,
    childCount: 1, // assume expandable
    assetCount: f.asset_count,
    location: { type: 'folder' as const, folderId: f.id, relPath: '' },
  }));
}

/**
 * Get child directory nodes within a source folder at a given rel_path prefix.
 */
export async function getFolderChildren(
  db: Database,
  folderId: number,
  parentRelPath: string,
): Promise<DirectoryNode[]> {
  // parentRelPath is '' for folder root, 'Packs' for a subfolder, etc.
  // We need to find the immediate child directories.
  const parentDepth = parentRelPath === '' ? 0 : parentRelPath.split('/').length;

  // Get all distinct rel_paths under this prefix (for non-zip assets)
  const relPathPattern = parentRelPath === '' ? '%' : parentRelPath + '/%';
  const [dirRows, zipRows] = await Promise.all([
    db.select<Array<{ rel_path: string; asset_count: number }>>(
      `SELECT rel_path, COUNT(*) as asset_count FROM assets
       WHERE folder_id = ? AND rel_path LIKE ? AND zip_file IS NULL
       GROUP BY rel_path
       ORDER BY rel_path COLLATE NOCASE`,
      [folderId, relPathPattern],
    ),
    // Find ZIP files at this level
    db.select<Array<{ rel_path: string; zip_file: string; asset_count: number }>>(
      `SELECT rel_path, zip_file, COUNT(*) as asset_count FROM assets
       WHERE folder_id = ? AND rel_path = ? AND zip_file IS NOT NULL
       GROUP BY rel_path, zip_file
       ORDER BY zip_file COLLATE NOCASE`,
      [folderId, parentRelPath],
    ),
  ]);

  // Build child directory nodes from rel_path segments
  const childMap = new Map<string, { assetCount: number; subDirs: Set<string> }>();

  for (const row of dirRows) {
    // Skip exact match (assets directly in parentRelPath — not subdirectories)
    if (row.rel_path === parentRelPath) continue;

    const segments = row.rel_path.split('/');
    if (segments.length <= parentDepth) continue;

    const childName = segments[parentDepth];
    const childRelPath = segments.slice(0, parentDepth + 1).join('/');

    if (!childMap.has(childRelPath)) {
      childMap.set(childRelPath, { assetCount: 0, subDirs: new Set() });
    }
    const entry = childMap.get(childRelPath)!;
    entry.assetCount += row.asset_count;

    // Track subdirectories for childCount
    if (segments.length > parentDepth + 1) {
      entry.subDirs.add(segments[parentDepth + 1]);
    }
  }

  const nodes: DirectoryNode[] = [];

  // Add filesystem directory children
  for (const [relPath, data] of childMap) {
    const name = relPath.split('/').pop()!;
    nodes.push({
      key: `folder:${folderId}:${relPath}`,
      name,
      childCount: data.subDirs.size,
      assetCount: data.assetCount,
      location: { type: 'folder', folderId, relPath },
    });
  }

  // Add ZIP file nodes
  for (const row of zipRows) {
    nodes.push({
      key: `zip:${folderId}:${row.rel_path}:${row.zip_file}`,
      name: row.zip_file,
      childCount: 1, // assume expandable
      assetCount: row.asset_count,
      location: { type: 'zip', folderId, relPath: row.rel_path, zipFile: row.zip_file, zipPrefix: '' },
    });
  }

  // Also check for ZIPs in child directories (they need to appear as expandable)
  // and count how many zips exist at each child rel_path level
  const zipInChildRows = await db.select<Array<{ rel_path: string; zip_count: number }>>(
    `SELECT rel_path, COUNT(DISTINCT zip_file) as zip_count FROM assets
     WHERE folder_id = ? AND rel_path LIKE ? AND rel_path != ? AND zip_file IS NOT NULL
     GROUP BY rel_path`,
    [folderId, relPathPattern, parentRelPath],
  );

  // Update childCount for directories that contain ZIPs
  for (const row of zipInChildRows) {
    const segments = row.rel_path.split('/');
    if (segments.length <= parentDepth) continue;
    const childRelPath = segments.slice(0, parentDepth + 1).join('/');
    const node = nodes.find((n) => n.location.type === 'folder' && n.location.relPath === childRelPath);
    if (node && node.childCount === 0) {
      node.childCount = 1; // has at least ZIP children
    }
  }

  return nodes.sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: 'base' }));
}

/**
 * Get child directories inside a ZIP file.
 */
export async function getZipDirectoryChildren(
  db: Database,
  folderId: number,
  relPath: string,
  zipFile: string,
  prefix: string,
): Promise<DirectoryNode[]> {
  // Get all zip_entry values under this prefix
  const likePattern = prefix === '' ? '%' : prefix + '%';
  const rows = await db.select<Array<{ zip_entry: string }>>(
    `SELECT zip_entry FROM assets
		WHERE folder_id = ? AND rel_path = ? AND zip_file = ? AND zip_entry IS NOT NULL AND zip_entry LIKE ?`,
    [folderId, relPath, zipFile, likePattern],
  );

  // Build directory structure from zip_entry paths
  const prefixDepth = prefix === '' ? 0 : prefix.split('/').filter(Boolean).length;
  const childMap = new Map<
    string,
    { assetCount: number; subDirs: Set<string>; hasDirectFiles: boolean; isNestedZip: boolean }
  >();

  for (const row of rows) {
    const entryPath = row.zip_entry;
    const segments = entryPath.split('/').filter(Boolean);
    if (segments.length <= prefixDepth) continue;

    const childName = segments[prefixDepth];

    if (segments.length === prefixDepth + 1) {
      // Direct file at current level — only show nested ZIPs as nodes
      if (childName.toLowerCase().endsWith('.zip')) {
        if (!childMap.has(childName)) {
          childMap.set(childName, { assetCount: 0, subDirs: new Set(), hasDirectFiles: false, isNestedZip: true });
        }
        childMap.get(childName)!.isNestedZip = true;
        childMap.get(childName)!.assetCount++;
      }
      continue;
    }

    if (!childMap.has(childName)) {
      childMap.set(childName, { assetCount: 0, subDirs: new Set(), hasDirectFiles: false, isNestedZip: false });
    }
    const entry = childMap.get(childName)!;
    entry.assetCount++;

    if (segments.length === prefixDepth + 2) {
      entry.hasDirectFiles = true;
    } else {
      entry.subDirs.add(segments[prefixDepth + 1]);
    }
  }

  return [...childMap.entries()]
    .filter(([_, data]) => data.subDirs.size > 0 || data.hasDirectFiles || data.isNestedZip)
    .map(([name, data]) => {
      const nodePrefix = prefix + name + '/';
      return {
        key: `zip:${folderId}:${relPath}:${zipFile}:${nodePrefix}`,
        name,
        childCount: data.subDirs.size,
        assetCount: data.assetCount,
        location: { type: 'zip' as const, folderId, relPath, zipFile, zipPrefix: nodePrefix },
      };
    })
    .sort((a, b) => a.name.localeCompare(b.name));
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
  folder_id: number;
  rel_path: string;
  zip_file: string | null;
  zip_entry: string | null;
  folder_path: string;
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
 * Find audio assets similar to a given audio asset using its stored CLAP embedding
 */
export async function searchAudioBySimilarity(
  assetId: number,
  limit: number = 500,
  durationFilter?: DurationFilter,
): Promise<SemanticSearchResult[]> {
  return invoke('search_audio_by_similarity', {
    assetId,
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
