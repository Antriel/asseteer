import { invoke } from '@tauri-apps/api/core';
import type { Asset, SearchQuery } from '$lib/types';

// Assets state - create a reactive state object
class AssetsState {
  assets = $state<Asset[]>([]);
  isLoading = $state(false);
  totalCount = $state(0);
  searchText = $state('');
  currentOffset = $state(0);
  pageSize = 100;

  /**
   * Load assets from the database
   */
  async loadAssets() {
    this.isLoading = true;

    try {
      const query: SearchQuery = {
        text: this.searchText || undefined,
        limit: this.pageSize,
        offset: this.currentOffset,
      };

      const result = await invoke<Asset[]>('search_assets', { query });
      this.assets = result;

      // Load total count
      const count = await invoke<number>('get_asset_count');
      this.totalCount = count;
    } catch (error) {
      console.error('Failed to load assets:', error);
    } finally {
      this.isLoading = false;
    }
  }

  /**
   * Search for assets
   */
  searchAssets(text: string) {
    this.searchText = text;
    this.currentOffset = 0;
    this.loadAssets();
  }
}

// Export singleton instance
export const assetsState = new AssetsState();

/**
 * Get formatted file size
 */
export function formatFileSize(bytes: number): string {
  if (bytes < 1024) return bytes + ' B';
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
  return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
}
