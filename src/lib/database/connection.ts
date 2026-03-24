import Database from '@tauri-apps/plugin-sql';

let db: Database | null = null;
let dbPromise: Promise<Database> | null = null;

/**
 * Get or create the database connection
 * Uses the preloaded database configured in tauri.conf.json
 */
export async function getDatabase(): Promise<Database> {
  if (db) return db;
  if (!dbPromise) {
    dbPromise = (async () => {
      console.time('[DB Frontend] Database.load');
      db = await Database.load('sqlite:asseteer.db');
      // Disable WAL auto-checkpoint — this read-only connection should never
      // trigger checkpoints (the backend manages them explicitly).
      await db.execute('PRAGMA wal_autocheckpoint=0', []);
      console.timeEnd('[DB Frontend] Database.load');
      return db;
    })();
  }
  return dbPromise;
}

/**
 * Close the database connection
 */
export async function closeDatabase(): Promise<void> {
  if (db) {
    await db.close();
    db = null;
    dbPromise = null;
  }
}
