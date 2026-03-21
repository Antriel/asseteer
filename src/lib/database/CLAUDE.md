# Frontend Database Layer

Direct SQLite access via `@tauri-apps/plugin-sql` for ALL read operations.

## Usage

```typescript
import { getDatabase } from '$lib/database/connection';
import { searchAssets, getAssetCount, getThumbnail } from '$lib/database/queries';

const db = await getDatabase();
const assets = await searchAssets(db, 'forest', 'image', 50, 0, undefined, null, 'anywhere');
const count = await getAssetCount(db);
```

## Available Query Functions

**Asset search & counts** (take `db` parameter):

| Function | Purpose |
|----------|---------|
| `searchAssets(db, searchText?, assetType?, limit, offset, durationFilter?, folderLocation?, searchColumn?)` | Search with FTS5 + pagination |
| `countSearchResults(db, searchText?, assetType?, durationFilter?, folderLocation?, searchColumn?)` | Count matching assets (same filters as search) |
| `getAssetCount(db)` | Total asset count |
| `getAssetCountByType(db, 'image'\|'audio')` | Count by type |
| `getAssetTypeCounts(db)` | Both image + audio counts |
| `getThumbnail(db, assetId)` | Get thumbnail as `Uint8Array` |
| `getPendingAssetCounts(db, preGenerateThumbnails?)` | Pending processing counts |

**Folder browsing** (take `db` parameter):

| Function | Purpose |
|----------|---------|
| `getSourceFolderRoots(db)` | Root directory nodes from active source folders |
| `getFolderChildren(db, folderId, parentRelPath)` | Child directories within a folder |
| `getZipDirectoryChildren(db, folderId, relPath, zipFile, prefix)` | Directories inside a ZIP |

**Search config** (take `db` parameter):

| Function | Purpose |
|----------|---------|
| `getSearchExcludes(db, folderId)` | Per-folder search exclusions |
| `getDistinctRelPaths(db, folderId)` | All rel_paths for folder tree |
| `getDistinctZipFiles(db, folderId)` | All ZIP files in a folder |
| `getDistinctZipDirs(db, folderId, relPath, zipFile)` | Directory prefixes inside a ZIP |

**CLAP / semantic search** (use `invoke()`, no `db` parameter):

| Function | Purpose |
|----------|---------|
| `searchAudioSemantic(query, limit?, durationFilter?, folderLocation?)` | Text→audio semantic search |
| `searchAudioBySimilarity(assetId, limit?, durationFilter?, folderLocation?)` | Find similar audio |
| `getPendingClapCount()` | Audio assets awaiting CLAP embedding |
| `checkClapServer()` / `startClapServer()` | Server management |
| `getClapServerInfo()` / `getClapCacheSize()` / `clearClapCache()` | Server info & cache |

## FTS5 Full-Text Search

Uses dual-table approach (handled internally by `buildFtsCondition()`):
- **Word table** (`assets_fts_word`): Prefix matching with `*` wildcard, works for any length
- **Trigram table** (`assets_fts_sub`): Exact substring matching, requires 3+ chars

Short queries (<3 chars) use word table only. Longer queries UNION both tables.

Supports column targeting via `SearchColumn`: `'anywhere'`, `'filename'`, `'path'`.

## BLOB Handling

`getThumbnail()` returns `Uint8Array` (converted internally). Use directly:

```typescript
const thumbnailData = await getThumbnail(db, assetId);
if (thumbnailData) {
  const blob = new Blob([thumbnailData], { type: 'image/webp' });
  const url = URL.createObjectURL(blob);
}
```

**ZIP asset bytes**: Use `loadAssetBlobUrl` from `$lib/utils/assetBlob` to load a ZIP-embedded asset as a blob URL. Do NOT call `invoke('get_asset_bytes')` directly in components:

```typescript
import { loadAssetBlobUrl } from '$lib/utils/assetBlob';

const url = await loadAssetBlobUrl(asset.id, `image/${asset.format}`);
// ...later, cleanup:
URL.revokeObjectURL(url);
```

## Key Rules

1. **All SELECTs go here** - Never create backend commands for reads
2. **Keep queries in `queries.ts`** - Centralized, typed, reusable. No ad-hoc `db.select()` calls in state modules or components.
3. **Backend only for writes** - INSERT/UPDATE/DELETE stay in Rust
4. **CLAP functions use `invoke()`** - Semantic search goes through Rust commands, not direct SQL
