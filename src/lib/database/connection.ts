import Database from '@tauri-apps/plugin-sql';
import { invoke } from '@tauri-apps/api/core';

let db: Database | null = null;
let dbPromise: Promise<Database> | null = null;

/**
 * Get or create the database connection.
 * Opens the exact file the backend writes (`get_db_path`), so the two can never
 * diverge — the harness's `--data-dir` relies on this.
 */
export async function getDatabase(): Promise<Database> {
  if (db) return db;
  if (!dbPromise) {
    dbPromise = (async () => {
      console.time('[DB Frontend] Database.load');
      const dbPath = await invoke<string>('get_db_path');
      db = await Database.load(`sqlite:${dbPath}`);
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
