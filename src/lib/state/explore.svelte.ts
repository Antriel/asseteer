import { SvelteMap } from 'svelte/reactivity';
import { getDatabase } from '$lib/database/connection';
import {
  getDirectoryChildren,
  getDirectoryRoots,
  getDirectoryRootCounts,
  getZipDirectoryChildren,
  ZIP_SEP,
} from '$lib/database/queries';

export type { DirectoryNode } from '$lib/database/queries';

class ExploreState {
  // Currently selected directory path
  selectedPath = $state<string | null>(null);
  // Expanded directory paths
  expandedPaths = $state(new SvelteMap<string, boolean>());
  // Cached children per directory path (null key = root level)
  childrenCache = $state(new SvelteMap<string, import('$lib/database/queries').DirectoryNode[]>());
  // Root directories (top-level scan roots)
  roots = $state<import('$lib/database/queries').DirectoryNode[]>([]);
  isLoadingRoots = $state(false);
  // True while navigateToAssetPath is expanding ancestors
  isNavigating = $state(false);

  async loadRoots(force = false) {
    if (this.isLoadingRoots) return;
    if (this.roots.length > 0 && !force) return;
    this.isLoadingRoots = true;
    try {
      const db = await getDatabase();
      // Phase 1: Show roots immediately without counts
      this.roots = await getDirectoryRoots(db);
      this.isLoadingRoots = false;

      // Phase 2: Load counts in background and update
      const counts = await getDirectoryRootCounts(db, this.roots);
      this.roots = counts;
    } catch (error) {
      console.error('[Explore] Failed to load roots:', error);
    } finally {
      this.isLoadingRoots = false;
    }
  }

  async loadChildren(path: string) {
    if (this.childrenCache.has(path)) return;
    try {
      const db = await getDatabase();
      const sepIdx = path.indexOf(ZIP_SEP);
      let children: import('$lib/database/queries').DirectoryNode[];
      if (sepIdx !== -1) {
        // ZIP-internal path: browse inside the zip
        const zipPath = path.substring(0, sepIdx);
        const prefix = path.substring(sepIdx + ZIP_SEP.length);
        children = await getZipDirectoryChildren(db, zipPath, prefix);
      } else if (path.toLowerCase().endsWith('.zip')) {
        // ZIP file node: browse its root
        children = await getZipDirectoryChildren(db, path, '');
      } else {
        // Regular filesystem directory
        children = await getDirectoryChildren(db, path);
      }
      this.childrenCache.set(path, children);
    } catch (error) {
      console.error('[Explore] Failed to load children for', path, error);
    }
  }

  async toggleExpanded(path: string) {
    const isExpanded = this.expandedPaths.get(path);
    if (isExpanded) {
      this.expandedPaths.set(path, false);
    } else {
      this.expandedPaths.set(path, true);
      await this.loadChildren(path);
    }
  }

  async navigateToAssetPath(assetPath: string, zipEntry?: string) {
    this.isNavigating = true;
    try {
      // Expand all ancestor directories and select the directory containing this asset
      const sep = assetPath.includes('\\') ? '\\' : '/';
      const parts = assetPath.split(sep);

      // Expand each ancestor on the filesystem
      let current = '';
      for (let i = 0; i < parts.length; i++) {
        current = i === 0 ? parts[i] : current + sep + parts[i];
        if (i === 0 && current.endsWith(':')) {
          continue;
        }
        if (!this.expandedPaths.get(current)) {
          this.expandedPaths.set(current, true);
          await this.loadChildren(current);
        }
      }

      // If there's a zip entry, navigate into the ZIP tree
      let targetPath = assetPath;
      if (zipEntry) {
        // Expand the ZIP file itself (uses ZIP_SEP encoding)
        const zipRootPath = assetPath; // The filesystem path to the .zip
        if (!this.expandedPaths.get(zipRootPath)) {
          this.expandedPaths.set(zipRootPath, true);
          await this.loadChildren(zipRootPath);
        }

        // Navigate through ZIP-internal directories
        // zip_entry is like "sounds/ambient/rain.wav" — we need to expand each directory segment
        const zipParts = zipEntry.split('/').filter(Boolean);
        // Remove the filename (last segment) — we want the containing directory
        const zipDirParts = zipParts.slice(0, -1);

        let zipPrefix = '';
        for (const part of zipDirParts) {
          zipPrefix += part + '/';
          const zipNodePath = assetPath + ZIP_SEP + zipPrefix;
          if (!this.expandedPaths.get(zipNodePath)) {
            this.expandedPaths.set(zipNodePath, true);
            await this.loadChildren(zipNodePath);
          }
        }

        // Select the deepest ZIP directory (or ZIP root if file is at root)
        targetPath = zipDirParts.length > 0 ? assetPath + ZIP_SEP + zipPrefix : assetPath;
      }

      // Select the target path
      this.selectedPath = targetPath;

      // Scroll into view after DOM updates
      requestAnimationFrame(() => {
        const el = document.querySelector('[data-tree-path][data-selected="true"]');
        el?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
      });
    } finally {
      this.isNavigating = false;
    }
  }

  getChildren(path: string): import('$lib/database/queries').DirectoryNode[] {
    return this.childrenCache.get(path) ?? [];
  }

  isExpanded(path: string): boolean {
    return this.expandedPaths.get(path) ?? false;
  }

  clearCache() {
    this.childrenCache.clear();
    this.roots = [];
  }
}

export const exploreState = new ExploreState();
