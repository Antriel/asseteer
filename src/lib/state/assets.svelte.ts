import type { Asset } from '$lib/types';
import { getDatabase } from '$lib/database/connection';
import { searchAssets as dbSearchAssets, getAssetCount } from '$lib/database/queries';

// Assets state - create a reactive state object
class AssetsState {
  assets = $state<Asset[]>([]);
  isLoading = $state(false);
  totalCount = $state(0);
  searchText = $state('');

  /**
   * Load assets from the database with optional type filter
   * Loads ALL assets since we use virtualization for efficient rendering
   */
  async loadAssets(assetType?: 'image' | 'audio') {
    this.isLoading = true;

    try {
      const db = await getDatabase();

      // Load all assets (no pagination needed with virtualization)
      // Using a high limit to effectively load all assets
      const result = await dbSearchAssets(
        db,
        this.searchText || undefined,
        assetType,
        999999, // High limit to load all assets
        0       // No offset
      );
      this.assets = result;

      // Load total count
      const count = await getAssetCount(db);
      this.totalCount = count;
    } catch (error) {
      console.error('Failed to load assets:', error);
    } finally {
      this.isLoading = false;
    }
  }

  /**
   * Search for assets with optional type filter
   */
  searchAssets(text: string, assetType?: 'image' | 'audio') {
    this.searchText = text;
    this.loadAssets(assetType);
  }

  /**
   * Get filtered assets by type (for derived computations)
   */
  getFilteredAssets(assetType: 'image' | 'audio'): Asset[] {
    return this.assets.filter(a => a.asset_type === assetType);
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
