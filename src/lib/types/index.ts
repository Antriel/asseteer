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

  // Processing state
  processing_status: string;
  processing_error: string | null;
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
