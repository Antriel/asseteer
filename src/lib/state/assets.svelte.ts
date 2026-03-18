import type { Asset } from '$lib/types';
import { getDatabase } from '$lib/database/connection';
import {
  searchAssets as dbSearchAssets,
  countSearchResults,
  getAssetCount,
  getAssetCountByType,
} from '$lib/database/queries';
import { clearThumbnailCache } from '$lib/state/thumbnails.svelte';

// Maximum number of assets to display in the UI
// Even with virtual scrolling, tracking millions of items causes performance issues
const MAX_DISPLAY_LIMIT = 5000;

// Duration filter type
export interface DurationFilter {
  minMs: number | null;
  maxMs: number | null;
}

// Assets state - create a reactive state object
class AssetsState {
  assets = $state<Asset[]>([]);
  isLoading = $state(false);
  totalCount = $state(0);
  searchText = $state('');
  // Track if there are more results than displayed
  hasMoreResults = $state(false);
  totalMatchingCount = $state(0);
  // Duration filter for audio assets
  durationFilter = $state<DurationFilter>({ minMs: null, maxMs: null });
  // Folder path filter (null = all folders)
  folderPath = $state<string | null>(null);

  // Search cancellation tracking
  private searchVersion = 0;

  /**
   * Load assets from the database with optional type filter
   * Only loads assets when there's a search query to avoid loading millions of items
   */
  async loadAssets(assetType?: 'image' | 'audio') {
    // Increment version to cancel any in-progress search
    const currentVersion = ++this.searchVersion;

    // Clear previous results and show loading state immediately
    this.assets = [];
    this.isLoading = true;
    this.hasMoreResults = false;
    clearThumbnailCache();

    try {
      const db = await getDatabase();

      // If no search text AND no folder filter, show empty state instead of loading all assets
      // This prevents loading 1M+ items which causes performance issues
      if (!this.searchText?.trim() && !this.folderPath) {
        if (currentVersion !== this.searchVersion) return;

        this.assets = [];
        this.hasMoreResults = false;

        // Still get the total count for the type so user knows how many exist
        this.totalMatchingCount = await getAssetCountByType(db, assetType || 'image');
        this.totalCount = await getAssetCount(db);
        return;
      }

      // Load assets with a sensible limit
      // Request one extra to detect if there are more results
      // Only apply duration filter for audio assets
      const durationFilter = assetType === 'audio' ? this.durationFilter : undefined;
      const result = await dbSearchAssets(
        db,
        this.searchText,
        assetType,
        MAX_DISPLAY_LIMIT + 1,
        0,
        durationFilter,
        this.folderPath,
      );

      // Only update if this search is still current
      if (currentVersion !== this.searchVersion) return;

      // Check if there are more results than we're displaying
      this.hasMoreResults = result.length > MAX_DISPLAY_LIMIT;

      if (this.hasMoreResults) {
        // Results were truncated - run a COUNT query to get the true total
        this.totalMatchingCount = await countSearchResults(
          db,
          this.searchText,
          assetType,
          durationFilter,
          this.folderPath,
        );
        // Re-check cancellation after the count query
        if (currentVersion !== this.searchVersion) return;
      } else {
        this.totalMatchingCount = result.length;
      }

      // Only keep up to MAX_DISPLAY_LIMIT
      this.assets = result.slice(0, MAX_DISPLAY_LIMIT);

      // Load total count
      const count = await getAssetCount(db);

      // Check again after count query
      if (currentVersion !== this.searchVersion) return;

      this.totalCount = count;
    } catch (error) {
      // Only log if this search is still current
      if (currentVersion === this.searchVersion) {
        console.error('Failed to load assets:', error);
      }
    } finally {
      // Only clear loading if this search is still current
      if (currentVersion === this.searchVersion) {
        this.isLoading = false;
      }
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
   * Set duration filter for audio assets
   */
  setDurationFilter(minMs: number | null, maxMs: number | null) {
    this.durationFilter = { minMs, maxMs };
  }

  /**
   * Set folder path filter and reload assets
   */
  setFolderFilter(path: string | null, assetType?: 'image' | 'audio') {
    this.folderPath = path;
    this.loadAssets(assetType);
  }

  /**
   * Get filtered assets by type (for derived computations)
   */
  getFilteredAssets(assetType: 'image' | 'audio'): Asset[] {
    return this.assets.filter((a) => a.asset_type === assetType);
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
