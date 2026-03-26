export interface Asset {
  id: number;
  filename: string;
  folder_id: number;
  rel_path: string;
  zip_file: string | null;
  zip_entry: string | null;
  folder_path: string; // transient, from JOIN with source_folders
  asset_type: string;
  format: string;
  file_size: number;

  // Image metadata
  width: number | null;
  height: number | null;

  // Audio metadata
  duration_ms: number | null;
  sample_rate: number | null;
  channels: number | null;

  // Timestamps
  created_at: number;
  modified_at: number;
}

export interface SourceFolder {
  id: number;
  path: string;
  label: string;
  added_at: number;
  last_scanned_at: number | null;
  asset_count: number;
  status: string;
  scan_warnings: string | null;
}

/** Structured folder location for filtering/navigation (replaces the old path::zipPrefix string) */
export type FolderLocation =
  | { type: 'folder'; folderId: number; relPath: string }
  | { type: 'zip'; folderId: number; relPath: string; zipFile: string; zipPrefix: string };

export interface SearchExclude {
  zip_file: string | null;
  excluded_path: string;
}

export interface ScanSession {
  id: number;
  source_folder_id: number | null;
  total_files: number | null;
  processed_files: number;
  status: string;
  started_at: number;
  completed_at: number | null;
  error: string | null;
}

export interface PendingCount {
  images: number;
  audio: number;
  clap: number;
  total: number;
}

export type ProcessingCategory = 'image' | 'audio' | 'clap';

export interface CategoryProgress {
  category: string;
  total: number;
  completed: number;
  failed: number;
  is_paused: boolean;
  is_running: boolean;
  // Processing details
  current_file: string | null;
  processing_rate: number;
  eta_seconds: number | null;
  // Computed properties (added in frontend)
  isPaused?: boolean;
  isRunning?: boolean;
}

export interface ProcessingErrorDetail {
  id: number;
  asset_id: number;
  filename: string;
  rel_path: string;
  zip_file: string | null;
  zip_entry: string | null;
  folder_path: string;
  error_message: string;
  occurred_at: number;
  retry_count: number;
}

// ============================================================================
// Path reconstruction helpers
// ============================================================================

/** Get the full filesystem path for a regular (non-ZIP) asset */
export function getAssetFilePath(asset: Asset): string {
  return asset.rel_path
    ? `${asset.folder_path}/${asset.rel_path}/${asset.filename}`
    : `${asset.folder_path}/${asset.filename}`;
}

/** Get the full filesystem path to the ZIP file containing a ZIP asset */
export function getAssetZipPath(asset: Asset): string {
  return asset.rel_path
    ? `${asset.folder_path}/${asset.rel_path}/${asset.zip_file}`
    : `${asset.folder_path}/${asset.zip_file}`;
}

/** Get a display-friendly location string for an asset */
export function getAssetDisplayPath(asset: Asset): string {
  if (asset.zip_entry) {
    return `${getAssetZipPath(asset)}/${asset.zip_entry}`;
  }
  return getAssetFilePath(asset);
}

/** Get the directory portion of the asset's display path (no filename) */
export function getAssetDirectoryPath(asset: Asset): string {
  if (asset.zip_entry) {
    const zipPath = getAssetZipPath(asset);
    const entryParts = asset.zip_entry.split('/');
    const internalDir = entryParts.slice(0, -1).join('/');
    return internalDir ? `${zipPath}/${internalDir}` : zipPath;
  }
  return asset.rel_path ? `${asset.folder_path}/${asset.rel_path}` : asset.folder_path;
}
