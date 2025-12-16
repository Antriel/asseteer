import type Database from '@tauri-apps/plugin-sql';
import type { Asset, PendingCount } from '$lib/types';

/**
 * Search for assets with optional full-text search and filtering
 */
export async function searchAssets(
	db: Database,
	searchText?: string,
	assetType?: string,
	limit: number = 50,
	offset: number = 0
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

	if (ftsQuery) {
		conditions.push('assets_fts MATCH ?');
		params.push(ftsQuery);
	}

	if (assetType) {
		conditions.push('assets.asset_type = ?');
		params.push(assetType);
	}

	const ftsJoin = ftsQuery ? 'INNER JOIN assets_fts ON assets.id = assets_fts.rowid' : '';
	const whereClause = conditions.length ? `WHERE ${conditions.join(' AND ')}` : '';

	const query = `
		${baseSelect}
		${ftsJoin}
		${joins}
		${whereClause}
		ORDER BY assets.filename COLLATE NOCASE ASC
		LIMIT ? OFFSET ?
	`;

	params.push(limit, offset);
	return db.select<Asset[]>(query, params);
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
 * Get count of pending assets that need processing
 */
export async function getPendingAssetCounts(db: Database): Promise<PendingCount> {
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

	const images = imagesResult[0]['COUNT(*)'];
	const audio = audioResult[0]['COUNT(*)'];

	return {
		images,
		audio,
		total: images + audio
	};
}
