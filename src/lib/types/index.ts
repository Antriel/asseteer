export interface Asset {
  id: number;
  filename: string;
  path: string;
  zip_entry: string | null;
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

export interface ScanSession {
  id: number;
  root_path: string;
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
  total: number;
}

export type ProcessingCategory = 'image' | 'audio';

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
  path: string;
  error_message: string;
  occurred_at: number;
  retry_count: number;
}
