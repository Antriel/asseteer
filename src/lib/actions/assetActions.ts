import type { Asset, FolderLocation } from '$lib/types';
import { getAssetDisplayPath } from '$lib/types';
import type { Attachment } from 'svelte/attachments';
import { openPath } from '@tauri-apps/plugin-opener';
import { invoke } from '@tauri-apps/api/core';
import { sep } from '@tauri-apps/api/path';
import { showToast } from '$lib/state/ui.svelte';
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
  const base = location.relPath
    ? join(normalize(folderBase), location.relPath)
    : normalize(folderBase);
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

/**
 * Copy the assets' full paths, one per line. A ZIP entry has no path of its own, so it
 * gets its location inside the archive (e.g. `D:\Packs\Retro.zip\Sounds\coin.wav`).
 */
export async function copyAssetPaths(assets: Asset[]) {
  const paths = assets.map((a) => getAssetDisplayPath(a).replace(/[\\/]/g, sep()));
  try {
    await navigator.clipboard.writeText(paths.join('\n'));
    showToast(paths.length === 1 ? 'Path copied' : `${paths.length} paths copied`, 'success');
  } catch (error) {
    showToast('Failed to copy path: ' + error, 'error');
  }
}

/** How far the pointer must travel with the button held before it counts as a drag. */
const DRAG_THRESHOLD_PX = 6;

/**
 * Drag assets out of the app as real files — into Audacity, a DAW, Explorer. The
 * backend extracts ZIP entries (and copies network files) only once a drag starts.
 *
 * `{@attach dragOut(() => selection.targets(assets, asset))}` on the row/tile: `targets`
 * is read when the drag starts, so it sees the selection as it is then. Plain clicks are
 * untouched: nothing happens until the pointer moves past the threshold with the primary
 * button held.
 */
export function dragOut(targets: () => Asset[]): Attachment<HTMLElement> {
  return (node) => {
    let startX = 0;
    let startY = 0;

    function onPointerMove(e: PointerEvent) {
      if (!(e.buttons & 1)) return stopTracking();
      if (Math.hypot(e.clientX - startX, e.clientY - startY) < DRAG_THRESHOLD_PX) return;
      stopTracking();
      startAssetDrag(targets());
    }

    function stopTracking() {
      window.removeEventListener('pointermove', onPointerMove);
      window.removeEventListener('pointerup', stopTracking);
    }

    function onPointerDown(e: PointerEvent) {
      if (e.button !== 0 || e.pointerType !== 'mouse') return;
      startX = e.clientX;
      startY = e.clientY;
      // On window: a quick flick can leave a 32px row before the first move event
      window.addEventListener('pointermove', onPointerMove);
      window.addEventListener('pointerup', stopTracking);
    }

    // Thumbnails are <img>, which the webview would start its own (useless) drag for
    function onDragStart(e: DragEvent) {
      e.preventDefault();
    }

    node.addEventListener('pointerdown', onPointerDown);
    node.addEventListener('dragstart', onDragStart);
    return () => {
      stopTracking();
      node.removeEventListener('pointerdown', onPointerDown);
      node.removeEventListener('dragstart', onDragStart);
    };
  };
}

async function startAssetDrag(assets: Asset[]) {
  if (assets.length === 0) return;
  try {
    await invoke<'dropped' | 'cancelled' | 'released'>('start_asset_drag', {
      assetIds: assets.map((a) => a.id),
      image: renderDragImage(assets),
    });
  } catch (error) {
    showToast('Drag failed: ' + error, 'error');
  }
}

/**
 * A small label under the cursor while dragging — the first file's name, plus how many
 * more come with it — as a PNG data URL.
 */
function renderDragImage(assets: Asset[]): string | null {
  const [asset] = assets;
  const dpr = window.devicePixelRatio || 1;
  const canvas = document.createElement('canvas');
  const ctx = canvas.getContext('2d');
  if (!ctx) return null;

  const font = `500 ${13 * dpr}px ui-sans-serif, system-ui, sans-serif`;
  const glyph = asset.asset_type === 'audio' ? '♪' : '▣';
  const text = asset.filename.length > 48 ? asset.filename.slice(0, 47) + '…' : asset.filename;
  ctx.font = font;
  const padX = 10 * dpr;
  const gap = 6 * dpr;
  const more = assets.length > 1 ? `+${assets.length - 1} more` : '';
  const glyphW = ctx.measureText(glyph).width;
  const textW = ctx.measureText(text).width;
  const moreW = more ? gap * 1.5 + ctx.measureText(more).width : 0;
  canvas.width = Math.ceil(padX * 2 + glyphW + gap + textW + moreW);
  canvas.height = Math.ceil(28 * dpr);

  // Colors from the current theme. Never pure black: Windows keys it out as transparent.
  const styles = getComputedStyle(document.documentElement);
  const color = (name: string, fallback: string) =>
    styles.getPropertyValue(name).trim() || fallback;

  ctx.fillStyle = color('--color-bg-elevated', '#ffffff');
  ctx.strokeStyle = color('--color-border-default', '#d4d4d8');
  ctx.lineWidth = dpr;
  ctx.beginPath();
  ctx.roundRect(dpr / 2, dpr / 2, canvas.width - dpr, canvas.height - dpr, 6 * dpr);
  ctx.fill();
  ctx.stroke();

  ctx.font = font;
  ctx.textBaseline = 'middle';
  ctx.fillStyle = color('--color-accent', '#3b82f6');
  ctx.fillText(glyph, padX, canvas.height / 2);
  ctx.fillStyle = color('--color-text-primary', '#18181b');
  ctx.fillText(text, padX + glyphW + gap, canvas.height / 2);
  if (more) {
    ctx.fillStyle = color('--color-text-secondary', '#57606a');
    ctx.fillText(more, padX + glyphW + gap + textW + gap * 1.5, canvas.height / 2);
  }

  return canvas.toDataURL('image/png');
}
