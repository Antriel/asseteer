import Database from '@tauri-apps/plugin-sql';

let db: Database | null = null;

/**
 * Get or create the database connection
 * Uses the preloaded database configured in tauri.conf.json
 */
export async function getDatabase(): Promise<Database> {
	if (!db) {
		db = await Database.load('sqlite:asseteer.db');
	}
	return db;
}

/**
 * Close the database connection
 */
export async function closeDatabase(): Promise<void> {
	if (db) {
		await db.close();
		db = null;
	}
}
