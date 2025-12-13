# Frontend Database Layer

Direct SQLite access via `@tauri-apps/plugin-sql` for ALL read operations.

## Usage

```typescript
import { getDatabase } from '$lib/database/connection';
import { searchAssets, getAssetCount, getThumbnail } from '$lib/database/queries';

const db = await getDatabase();
const assets = await searchAssets(db, 'forest', 'image', 50, 0);
const count = await getAssetCount(db);
```

## Available Query Functions

| Function | Purpose |
|----------|---------|
| `searchAssets(db, searchText?, assetType?, limit, offset)` | Search with FTS5 + pagination |
| `getAssetCount(db)` | Total asset count |
| `getThumbnail(db, assetId)` | Get thumbnail BLOB |
| `getPendingAssetCounts(db)` | Pending processing counts |

## BLOB Handling

SQL plugin returns `number[]`. Convert to `Uint8Array` for Blob:

```typescript
const thumbnailData = await getThumbnail(db, assetId);
if (thumbnailData) {
  const blob = new Blob([thumbnailData], { type: 'image/jpeg' });
  const url = URL.createObjectURL(blob);
}
```

## FTS5 Full-Text Search

Add `*` wildcard for prefix matching:

```typescript
// In queries.ts
const ftsQuery = `${searchText}*`;  // "for" matches "forest", "format", etc.
```

## Key Rules

1. **All SELECTs go here** - Never create backend commands for reads
2. **Keep queries in `queries.ts`** - Centralized, typed, reusable
3. **Backend only for writes** - INSERT/UPDATE/DELETE stay in Rust
