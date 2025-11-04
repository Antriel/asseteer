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

export interface ProcessingTask {
  id: number;
  asset_id: number;
  task_type: string;
  status: string;
  priority: number;
  retry_count: number;
  max_retries: number;

  // Timestamps
  created_at: number;
  started_at: number | null;
  completed_at: number | null;

  // Error tracking
  error_message: string | null;

  // Progress
  progress_current: number;
  progress_total: number;

  // Task-specific data
  input_params: string | null;
  output_data: string | null;
}

export interface TaskProgress {
  task_id: number;
  asset_id: number;
  task_type: string;
  status: string;
  progress_current: number;
  progress_total: number;
  current_file: string;
}

export interface TaskStats {
  total: number;
  pending: number;
  queued: number;
  processing: number;
  paused: number;
  complete: number;
  error: number;
  cancelled: number;
}

export interface SearchQuery {
  text?: string;
  asset_type?: string;
  limit: number;
  offset: number;
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
