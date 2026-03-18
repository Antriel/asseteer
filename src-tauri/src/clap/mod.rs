//! CLAP (Contrastive Language-Audio Pretraining) integration module
//!
//! Provides async HTTP client for the Python CLAP server and embedding utilities.

mod client;
mod embedding;
mod job_object;
mod logs;
mod server;
pub mod uv;

use once_cell::sync::OnceCell;
use tauri::AppHandle;

static APP_HANDLE: OnceCell<AppHandle> = OnceCell::new();

/// Store the app handle for emitting events from the CLAP module.
/// Must be called once during app setup.
pub fn init_app_handle(handle: AppHandle) {
    let _ = APP_HANDLE.set(handle);
}

/// Get the stored app handle, if available.
pub(crate) fn get_app_handle() -> Option<&'static AppHandle> {
    APP_HANDLE.get()
}

pub use client::{get_clap_client, HealthInfo};
pub use embedding::{blob_to_embedding, cosine_similarity, embedding_to_blob};
pub use logs::log_dir;
pub use server::{ensure_server_running, stop_server, stop_server_and_wait};
