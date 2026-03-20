import { SvelteMap } from 'svelte/reactivity';
import { getDatabase } from '$lib/database/connection';
import {
  getSourceFolderRoots,
  getFolderChildren,
  getZipDirectoryChildren,
} from '$lib/database/queries';
import type { FolderLocation, Asset } from '$lib/types';

export type { DirectoryNode } from '$lib/database/queries';

class ExploreState {
  // Currently selected node key
  selectedKey = $state<string | null>(null);
  // Currently selected location (for filtering)
  selectedLocation = $state<FolderLocation | null>(null);
  // Expanded node keys
  expandedKeys = $state(new SvelteMap<string, boolean>());
  // Cached children per node key
  childrenCache = $state(new SvelteMap<string, import('$lib/database/queries').DirectoryNode[]>());
  // Root directories (source folders)
  roots = $state<import('$lib/database/queries').DirectoryNode[]>([]);
  // Maps folderId -> absolute folder path (populated alongside roots)
  folderPaths = $state(new Map<number, string>());
  isLoadingRoots = $state(false);
  // True while navigateToAsset is expanding ancestors
  isNavigating = $state(false);

  async loadRoots(force = false) {
    if (this.isLoadingRoots) return;
    if (this.roots.length > 0 && !force) return;
    this.isLoadingRoots = true;
    try {
      const db = await getDatabase();
      this.roots = await getSourceFolderRoots(db);
      const folderRows = await db.select<{ id: number; path: string }[]>(
        `SELECT id, path FROM source_folders WHERE status = 'active'`,
      );
      this.folderPaths = new Map(folderRows.map((f) => [f.id, f.path]));
    } catch (error) {
      console.error('[Explore] Failed to load roots:', error);
    } finally {
      this.isLoadingRoots = false;
    }
  }

  async loadChildren(key: string, location: FolderLocation) {
    if (this.childrenCache.has(key)) return;
    try {
      const db = await getDatabase();
      let children: import('$lib/database/queries').DirectoryNode[];

      if (location.type === 'zip') {
        children = await getZipDirectoryChildren(
          db,
          location.folderId,
          location.relPath,
          location.zipFile,
          location.zipPrefix,
        );
      } else {
        children = await getFolderChildren(db, location.folderId, location.relPath);
      }
      this.childrenCache.set(key, children);
    } catch (error) {
      console.error('[Explore] Failed to load children for', key, error);
    }
  }

  async toggleExpanded(key: string, location: FolderLocation) {
    const isExpanded = this.expandedKeys.get(key);
    if (isExpanded) {
      this.expandedKeys.set(key, false);
    } else {
      this.expandedKeys.set(key, true);
      await this.loadChildren(key, location);
    }
  }

  /**
   * Navigate the tree to reveal and select the node containing a given asset.
   */
  async navigateToAsset(asset: Asset) {
    this.isNavigating = true;
    try {
      const db = await getDatabase();
      const folderId = asset.folder_id;

      // Find and expand the root node for this folder
      const rootKey = `folder:${folderId}`;
      if (!this.expandedKeys.get(rootKey)) {
        this.expandedKeys.set(rootKey, true);
        await this.loadChildren(rootKey, { type: 'folder', folderId, relPath: '' });
      }

      // Expand each segment of rel_path
      if (asset.rel_path) {
        const segments = asset.rel_path.split('/');
        let currentRelPath = '';
        for (const segment of segments) {
          currentRelPath = currentRelPath ? `${currentRelPath}/${segment}` : segment;
          const nodeKey = `folder:${folderId}:${currentRelPath}`;
          if (!this.expandedKeys.get(nodeKey)) {
            this.expandedKeys.set(nodeKey, true);
            await this.loadChildren(nodeKey, { type: 'folder', folderId, relPath: currentRelPath });
          }
        }
      }

      // If it's a ZIP asset, expand the ZIP node and its internal directories
      if (asset.zip_file && asset.zip_entry) {
        const zipKey = `zip:${folderId}:${asset.rel_path}:${asset.zip_file}`;
        if (!this.expandedKeys.get(zipKey)) {
          this.expandedKeys.set(zipKey, true);
          await this.loadChildren(zipKey, {
            type: 'zip', folderId, relPath: asset.rel_path, zipFile: asset.zip_file, zipPrefix: '',
          });
        }

        // Expand zip-internal directories
        const zipParts = asset.zip_entry.split('/').filter(Boolean);
        const zipDirParts = zipParts.slice(0, -1); // remove filename
        let zipPrefix = '';
        for (const part of zipDirParts) {
          zipPrefix += part + '/';
          const zipNodeKey = `zip:${folderId}:${asset.rel_path}:${asset.zip_file}:${zipPrefix}`;
          if (!this.expandedKeys.get(zipNodeKey)) {
            this.expandedKeys.set(zipNodeKey, true);
            await this.loadChildren(zipNodeKey, {
              type: 'zip', folderId, relPath: asset.rel_path, zipFile: asset.zip_file, zipPrefix,
            });
          }
        }

        // Select the deepest ZIP directory (or ZIP root if file is at root)
        if (zipDirParts.length > 0) {
          this.selectedKey = `zip:${folderId}:${asset.rel_path}:${asset.zip_file}:${zipPrefix}`;
          this.selectedLocation = {
            type: 'zip', folderId, relPath: asset.rel_path, zipFile: asset.zip_file, zipPrefix,
          };
        } else {
          this.selectedKey = zipKey;
          this.selectedLocation = {
            type: 'zip', folderId, relPath: asset.rel_path, zipFile: asset.zip_file, zipPrefix: '',
          };
        }
      } else {
        // Regular file — select the directory
        const targetRelPath = asset.rel_path || '';
        const targetKey = targetRelPath ? `folder:${folderId}:${targetRelPath}` : rootKey;
        this.selectedKey = targetKey;
        this.selectedLocation = { type: 'folder', folderId, relPath: targetRelPath };
      }

      // Scroll into view after DOM updates
      requestAnimationFrame(() => {
        const el = document.querySelector('[data-tree-key][data-selected="true"]');
        el?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
      });
    } finally {
      this.isNavigating = false;
    }
  }

  getChildren(key: string): import('$lib/database/queries').DirectoryNode[] {
    return this.childrenCache.get(key) ?? [];
  }

  isExpanded(key: string): boolean {
    return this.expandedKeys.get(key) ?? false;
  }

  clearCache() {
    this.childrenCache.clear();
    this.roots = [];
    this.folderPaths = new Map();
  }
}

export const exploreState = new ExploreState();
