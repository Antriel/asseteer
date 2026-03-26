import type { Asset, FolderLocation } from '$lib/types';
import { openPath } from '@tauri-apps/plugin-opener';
import { sep } from '@tauri-apps/api/path';
import { viewState } from '$lib/state/view.svelte';
import { assetsState } from '$lib/state/assets.svelte';
import { exploreState } from '$lib/state/explore.svelte';

export async function showInFolder(asset: Asset, assetType: 'image' | 'audio') {
  viewState.openFolderSidebar();
  await exploreState.loadRoots();
  await exploreState.navigateToAsset(asset);
  let location: FolderLocation;
  if (asset.zip_file) {
    const zipParts = (asset.zip_entry ?? '').split('/').filter(Boolean);
    const zipDirParts = zipParts.slice(0, -1);
    const zipPrefix = zipDirParts.length > 0 ? zipDirParts.join('/') + '/' : '';
    location = {
      type: 'zip',
      folderId: asset.folder_id,
      relPath: asset.rel_path,
      zipFile: asset.zip_file,
      zipPrefix,
    };
  } else {
    location = { type: 'folder', folderId: asset.folder_id, relPath: asset.rel_path };
  }
  assetsState.setFolderFilter(location, assetType);
}

export async function openLocationInExplorer(folderBase: string, location: FolderLocation) {
  const join = (...parts: string[]) => parts.join(sep());
  const normalize = (p: string) => p.replace(/[\\/]/g, sep());
  const base = location.relPath ? join(normalize(folderBase), location.relPath) : normalize(folderBase);
  let dirPath: string;
  if (location.type === 'zip') {
    const zipParts = location.zipFile.split('/');
    const outerZip = zipParts[0];
    const prefixParts = location.zipPrefix ? location.zipPrefix.replace(/\/$/, '').split('/') : [];
    const prefixHasInnerZip = prefixParts.some((p) => /\.zip$/i.test(p));
    if (zipParts.length === 1 && prefixParts.length > 0 && !prefixHasInnerZip) {
      // Single zip, no inner zips in path: navigate into the zip to the represented directory
      dirPath = join(base, outerZip, normalize(prefixParts.join('/')));
    } else {
      // Nested zip (in zipFile or zipPrefix): stop at the outermost real filesystem zip
      dirPath = join(base, outerZip);
    }
  } else {
    dirPath = base;
  }
  try {
    await openPath(dirPath);
  } catch (error) {
    console.error('Failed to open in explorer:', error);
  }
}

export async function openDirectory(asset: Asset) {
  if (asset.zip_file) {
    const entryParts = (asset.zip_entry ?? '').split('/').filter(Boolean);
    const zipPrefix = entryParts.length > 1 ? entryParts.slice(0, -1).join('/') + '/' : '';
    const location: FolderLocation = {
      type: 'zip',
      folderId: asset.folder_id,
      relPath: asset.rel_path,
      zipFile: asset.zip_file,
      zipPrefix,
    };
    await openLocationInExplorer(asset.folder_path, location);
  } else {
    const location: FolderLocation = {
      type: 'folder',
      folderId: asset.folder_id,
      relPath: asset.rel_path,
    };
    await openLocationInExplorer(asset.folder_path, location);
  }
}
