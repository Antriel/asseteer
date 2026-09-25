//! One-time data fixes for existing libraries, tracked in `PRAGMA user_version`.
//!
//! They run in the background once the window is up (`start`), while the frontend's
//! `DbMigrationDialog` polls `get_db_migration` and blocks the UI until they finish.

use serde::Serialize;
use sqlx::SqlitePool;
use std::sync::Mutex;

/// Bump when adding a step to `run`.
pub const DATA_VERSION: i64 = 1;

#[derive(Clone, Serialize)]
pub struct MigrationProgress {
    pub message: String,
    pub done: u64,
    /// 0 while the amount of work isn't known yet
    pub total: u64,
    /// Set when the migration failed; it is retried on the next start
    pub error: Option<String>,
}

static PROGRESS: Mutex<Option<MigrationProgress>> = Mutex::new(None);

/// The running (or failed) migration, `None` once there is nothing to wait for.
pub fn progress() -> Option<MigrationProgress> {
    PROGRESS.lock().unwrap().clone()
}

fn set_progress(progress: Option<MigrationProgress>) {
    *PROGRESS.lock().unwrap() = progress;
}

async fn user_version(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    let (version,): (i64,) = sqlx::query_as("PRAGMA user_version")
        .fetch_one(pool)
        .await?;
    Ok(version)
}

async fn set_user_version(pool: &SqlitePool, version: i64) -> Result<(), sqlx::Error> {
    sqlx::query(&format!("PRAGMA user_version = {}", version))
        .execute(pool)
        .await?;
    Ok(())
}

/// Whether `run` has work to do. A library without assets has nothing to fix, so it
/// is stamped current here and never shows the dialog.
pub async fn pending(pool: &SqlitePool) -> Result<bool, sqlx::Error> {
    if user_version(pool).await? >= DATA_VERSION {
        return Ok(false);
    }
    let (has_assets,): (bool,) = sqlx::query_as("SELECT EXISTS(SELECT 1 FROM assets)")
        .fetch_one(pool)
        .await?;
    if !has_assets {
        set_user_version(pool, DATA_VERSION).await?;
    }
    Ok(has_assets)
}

/// Run the migrations on a background task. Progress is visible from the moment this
/// returns, so the frontend's first poll can't miss it.
pub fn start(pool: SqlitePool) {
    set_progress(Some(MigrationProgress {
        message: "Preparing…".into(),
        done: 0,
        total: 0,
        error: None,
    }));
    tauri::async_runtime::spawn(async move {
        let started = std::time::Instant::now();
        let result = run(&pool, |message, done, total| {
            set_progress(Some(MigrationProgress {
                message: message.into(),
                done,
                total,
                error: None,
            }))
        })
        .await;
        match result {
            Ok(()) => {
                println!("[DB] Data migration done in {:.1?}", started.elapsed());
                set_progress(None);
            }
            Err(e) => {
                eprintln!("[DB] Data migration failed: {}", e);
                set_progress(Some(MigrationProgress {
                    message: String::new(),
                    done: 0,
                    total: 0,
                    error: Some(e),
                }));
            }
        }
    });
}

/// Apply every step newer than the library's `user_version`, stamping each as it
/// completes so an interrupted run resumes where it stopped.
pub async fn run(
    pool: &SqlitePool,
    mut report: impl FnMut(&str, u64, u64) + Send,
) -> Result<(), String> {
    let version = user_version(pool).await.map_err(|e| e.to_string())?;

    if version < 1 {
        // searchable_path gained the outer zip's name (asseteer-8765)
        const MESSAGE: &str = "Re-indexing search paths of files inside zips";
        let (total,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM assets WHERE zip_file IS NOT NULL")
                .fetch_one(pool)
                .await
                .map_err(|e| e.to_string())?;
        let total = total as u64;
        let folders: Vec<(i64,)> = sqlx::query_as("SELECT id FROM source_folders")
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;
        let mut done = 0;
        report(MESSAGE, done, total);
        for (folder_id,) in folders {
            crate::commands::folders::reindex_searchable_paths(pool, folder_id, true, |n| {
                done += n;
                report(MESSAGE, done, total);
            })
            .await?;
        }
        report("Finishing up…", total, total);
        super::checkpoint_truncate(pool)
            .await
            .map_err(|e| e.to_string())?;
        set_user_version(pool, 1).await.map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{
        create_test_db_file, insert_asset, insert_source_folder, make_asset,
    };

    #[tokio::test]
    async fn empty_library_is_stamped_current() {
        let dir = tempfile::tempdir().unwrap();
        let pool = create_test_db_file(dir.path().join("m.db").to_str().unwrap()).await;
        assert!(!pending(&pool).await.unwrap());
        assert_eq!(user_version(&pool).await.unwrap(), DATA_VERSION);
    }

    #[tokio::test]
    async fn v1_indexes_outer_zip_name() {
        let dir = tempfile::tempdir().unwrap();
        let pool = create_test_db_file(dir.path().join("m.db").to_str().unwrap()).await;
        let folder = insert_source_folder(&pool, "C:/lib", "lib").await;

        // A zip asset indexed the old way, before the zip's name was a segment
        let mut asset = make_asset("retro_coin.wav", folder, "Packs", "audio", "wav");
        asset.zip_file = Some("Retro Pack.zip".into());
        asset.zip_entry = Some("Sounds/retro_coin.wav".into());
        let id = insert_asset(&pool, &asset).await;
        sqlx::query("UPDATE assets SET searchable_path = 'Packs Sounds' WHERE id = ?")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();

        assert!(pending(&pool).await.unwrap());
        let mut reports = Vec::new();
        run(&pool, |_, done, total| reports.push((done, total)))
            .await
            .unwrap();
        assert_eq!(reports.last(), Some(&(1, 1)));

        let (sp,): (String,) = sqlx::query_as("SELECT searchable_path FROM assets WHERE id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(sp, "Packs Retro Pack Sounds");
        let (hits,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM assets_fts_word WHERE assets_fts_word MATCH 'searchable_path:retro'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(hits, 1);
        assert!(!pending(&pool).await.unwrap());
    }
}
