import { invoke } from '@tauri-apps/api/core';

/**
 * Load a ZIP-embedded asset as a blob URL.
 * Uses binary IPC (ArrayBuffer) to avoid JSON number[] overhead.
 * Caller is responsible for calling URL.revokeObjectURL() when done.
 */
export async function loadAssetBlobUrl(assetId: number, mimeType: string): Promise<string> {
  const buffer = await invoke<ArrayBuffer>('get_asset_bytes', { assetId });
  const blob = new Blob([buffer], { type: mimeType });
  return URL.createObjectURL(blob);
}
