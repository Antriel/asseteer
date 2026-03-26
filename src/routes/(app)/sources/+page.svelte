<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { emit, listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { onMount, onDestroy } from 'svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import { openPath } from '@tauri-apps/plugin-opener';
  import { showToast, showConfirm, uiState } from '$lib/state/ui.svelte';
  import { formatElapsed } from '$lib/state/tasks.svelte';
  import { exploreState } from '$lib/state/explore.svelte';
  import { assetsState } from '$lib/state/assets.svelte';
  import { processingState } from '$lib/state/tasks.svelte';
  import { viewState } from '$lib/state/view.svelte';
  import type { SourceFolder } from '$lib/types';
  import Spinner from '$lib/components/shared/Spinner.svelte';
  import FolderIcon from '$lib/components/icons/FolderIcon.svelte';
  import SearchConfigPanel from '$lib/components/folders/SearchConfigPanel.svelte';

  interface ScanProgressEvent {
    phase: string;
    files_found: number;
    files_inserted: number;
    files_total: number;
    zips_scanned: number;
    current_path: string | null;
    warnings?: string[];
    folder_path?: string;
  }

  interface RescanPreviewResult {
    preview_id: string;
    added_count: number;
    removed_count: number;
    modified_count: number;
    unchanged_count: number;
    warnings?: string[];
  }

  interface RescanApplyResult {
    inserted: number;
    deleted: number;
    updated: number;
  }

  // Rescan state for a folder
  type RescanPhase = 'scanning' | 'preview' | 'applying' | 'done';

  interface RescanState {
    phase: RescanPhase;
    progress: string;
    preview: RescanPreviewResult | null;
    applyResult: RescanApplyResult | null;
  }

  let folders = $state<SourceFolder[]>([]);
  let loading = $state(true);
  let editingId = $state<number | null>(null);
  let editLabel = $state('');
  let editInput = $state<HTMLInputElement | null>(null);

  // Single shared listener for all scan progress events
  let scanUnlisten: UnlistenFn | null = null;
  let unlistenScanComplete: UnlistenFn | null = null;

  let rescanUnlisten: UnlistenFn | null = null;

  // Per-folder rescan state (only one at a time)
  let rescanFolderId = $state<number | null>(null);
  let rescanState = $state<RescanState | null>(null);

  // Folder removal state
  let removingId = $state<number | null>(null);
  let removeProgress = $state('');
  let removeUnlisten: UnlistenFn | null = null;

  onMount(async () => {
    await loadFolders();
    // Set up a single shared listener for scan-progress events
    scanUnlisten = await listen<ScanProgressEvent>('scan-progress', (event) => {
      const e = event.payload;
      const folderPath = e.folder_path;
      if (!folderPath) return;
      // Only update if we're tracking this scan
      if (!uiState.isScanningFolder(folderPath)) return;
      applyScanProgress(folderPath, e);
    });

    // Reload folders when a scan completes — needed when navigating back mid-scan
    unlistenScanComplete = await listen('scan-complete', async () => {
      await loadFolders();
    });
  });

  onDestroy(() => {
    if (scanUnlisten) {
      scanUnlisten();
      scanUnlisten = null;
    }
    if (rescanUnlisten) {
      rescanUnlisten();
      rescanUnlisten = null;
    }
    if (unlistenScanComplete) {
      unlistenScanComplete();
      unlistenScanComplete = null;
    }
    if (removeUnlisten) {
      removeUnlisten();
      removeUnlisten = null;
    }
  });

  async function loadFolders() {
    loading = true;
    try {
      folders = await invoke<SourceFolder[]>('list_folders');
    } catch (error) {
      showToast('Failed to load folders: ' + error, 'error');
    } finally {
      loading = false;
    }
  }

  function formatDate(timestamp: number | null): string {
    if (!timestamp) return 'Never';
    return new Date(timestamp * 1000).toLocaleDateString(undefined, {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    });
  }

  function formatCount(count: number): string {
    if (count >= 1000) return (count / 1000).toFixed(1) + 'k';
    return count.toString();
  }

  // Rename
  function startRename(folder: SourceFolder) {
    editingId = folder.id;
    editLabel = folder.label || folderName(folder.path);
    requestAnimationFrame(() => editInput?.select());
  }

  async function saveRename(folder: SourceFolder) {
    const newLabel = editLabel.trim();
    editingId = null;
    if (!newLabel || newLabel === folder.label) return;

    try {
      await invoke('rename_folder', { folderId: folder.id, label: newLabel });
      folder.label = newLabel;
      exploreState.clearCache();
      await exploreState.loadRoots(true);
    } catch (error) {
      showToast('Failed to rename: ' + error, 'error');
    }
  }

  function cancelRename() {
    editingId = null;
  }

  function handleRenameKeydown(event: KeyboardEvent, folder: SourceFolder) {
    if (event.key === 'Enter') saveRename(folder);
    else if (event.key === 'Escape') cancelRename();
  }

  // Remove
  async function removeFolder(folder: SourceFolder) {
    const name = folder.label || folderName(folder.path);
    const confirmed = await showConfirm(
      `Remove "${name}" from the library? ${folder.asset_count.toLocaleString()} assets will be unindexed. Files on disk are not affected.`,
      'Remove Folder',
      'Remove',
    );
    if (!confirmed) return;

    removingId = folder.id;
    removeProgress = 'Removing...';

    removeUnlisten = await listen<{ phase: string; deleted: number; total: number }>(
      'folder-remove-progress',
      (event) => {
        const { phase, deleted, total } = event.payload;
        if (phase === 'deleting' && total > 0) {
          const pct = Math.round((deleted / total) * 100);
          removeProgress = `Removing from library... ${deleted.toLocaleString()}/${total.toLocaleString()} (${pct}%)`;
        } else if (phase === 'compacting') {
          removeProgress = 'Finishing up...';
        }
      },
    );

    try {
      await invoke('remove_folder', { folderId: folder.id });
      folders = folders.filter((f) => f.id !== folder.id);
      showToast(`Removed "${name}"`, 'success');
      exploreState.clearCache();
      await exploreState.loadRoots(true);
      const currentType = viewState.activeTab === 'images' ? 'image' : 'audio';
      await assetsState.loadAssets(currentType);
    } catch (error) {
      showToast('Failed to remove folder: ' + error, 'error');
    } finally {
      removingId = null;
      removeProgress = '';
      if (removeUnlisten) {
        removeUnlisten();
        removeUnlisten = null;
      }
    }
  }

  function applyScanProgress(folderPath: string, e: ScanProgressEvent) {
    let progressMessage = '';
    const details = {
      phase: e.phase as 'idle' | 'discovering' | 'scanning' | 'inserting' | 'indexing' | 'complete',
      filesFound: e.files_found,
      filesInserted: e.files_inserted,
      filesTotal: e.files_total,
      zipsScanned: e.zips_scanned,
      currentPath: e.current_path ?? uiState.activeScans.get(folderPath)?.details.currentPath ?? null,
    };

    if (e.phase === 'discovering') {
      const zipInfo = e.zips_scanned > 0 ? ` (${e.zips_scanned} zips)` : '';
      progressMessage = `Discovering... ${e.files_found} found${zipInfo}`;
    } else if (e.phase === 'inserting' || e.phase === 'scanning') {
      if (e.files_total > 0) {
        const pct = Math.round((e.files_inserted / e.files_total) * 100);
        progressMessage = `Saving... ${e.files_inserted.toLocaleString()}/${e.files_total.toLocaleString()} (${pct}%)`;
      } else {
        progressMessage = `Scanning... ${e.files_found.toLocaleString()} found`;
      }
    } else if (e.phase === 'indexing') {
      const pct = Math.round((e.files_inserted / e.files_total) * 100);
      progressMessage = `Indexing for search... ${e.files_inserted.toLocaleString()}/${e.files_total.toLocaleString()} (${pct}%)`;
    } else {
      progressMessage = `Done! ${e.files_found} assets.`;
      if (e.warnings && e.warnings.length > 0) {
        const count = e.warnings.length;
        const msg =
          count === 1
            ? `Warning: ${e.warnings[0]}`
            : `${count} files could not be read during scan`;
        showToast(msg, 'warning', 8000);
      }
    }

    uiState.updateScan(folderPath, progressMessage, details);
  }

  // Add folder — supports concurrent scans
  async function addFolder() {
    let succeeded = false;
    let selectedPath: string | null = null;
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select folder to add',
      });
      if (!selected || typeof selected !== 'string') return;

      selectedPath = selected.replace(/\\/g, '/');
      uiState.startScan(selectedPath);

      await invoke('add_folder', { path: selected });
      showToast('Folder added successfully', 'success');
      succeeded = true;
    } catch (error) {
      showToast('Failed to add folder: ' + error, 'error');
    } finally {
      if (selectedPath) {
        uiState.endScan(selectedPath);
      }
    }

    // Reload folder list after scan ends so the new card appears immediately
    await loadFolders();
    if (succeeded) {
      exploreState.clearCache();
      await exploreState.loadRoots(true);
      await processingState.refreshPendingCount();
      await emit('scan-complete');
    }
  }

  // Rescan - Preview phase
  async function startRescan(folder: SourceFolder) {
    if (rescanFolderId !== null) return; // One at a time

    rescanFolderId = folder.id;
    rescanState = {
      phase: 'scanning',
      progress: 'Scanning filesystem...',
      preview: null,
      applyResult: null,
    };

    if (rescanUnlisten) {
      rescanUnlisten();
      rescanUnlisten = null;
    }

    rescanUnlisten = await listen<ScanProgressEvent>('rescan-progress', (event) => {
      const e = event.payload;
      if (e.phase === 'scanning') {
        const zipInfo = e.zips_scanned > 0 ? ` (${e.zips_scanned} zips)` : '';
        rescanState = {
          ...rescanState!,
          progress: `Scanning... ${e.files_found} files found${zipInfo}`,
        };
      }
    });

    try {
      const preview = await invoke<RescanPreviewResult>('preview_rescan', { folderId: folder.id });

      if (rescanUnlisten) {
        rescanUnlisten();
        rescanUnlisten = null;
      }

      if (preview.warnings && preview.warnings.length > 0) {
        const count = preview.warnings.length;
        const msg =
          count === 1
            ? `Warning: ${preview.warnings[0]}`
            : `${count} files could not be read during rescan`;
        showToast(msg, 'warning', 8000);
      }

      // If nothing changed, skip the preview step
      if (
        preview.added_count === 0 &&
        preview.removed_count === 0 &&
        preview.modified_count === 0
      ) {
        rescanState = { phase: 'done', progress: '', preview, applyResult: null };
        showToast('Folder is up to date — no changes found', 'info');
        setTimeout(dismissRescan, 3000);
        return;
      }

      rescanState = { phase: 'preview', progress: '', preview, applyResult: null };
    } catch (error) {
      showToast('Rescan failed: ' + error, 'error');
      dismissRescan();
    }
  }

  // Rescan - Apply phase
  async function applyRescan(folder: SourceFolder) {
    if (!rescanState || rescanState.phase !== 'preview') return;

    rescanState = { ...rescanState, phase: 'applying', progress: 'Applying changes...' };

    if (rescanUnlisten) {
      rescanUnlisten();
      rescanUnlisten = null;
    }

    rescanUnlisten = await listen<ScanProgressEvent>('rescan-progress', (event) => {
      const e = event.payload;
      if (e.phase === 'applying' && e.files_total > 0) {
        const pct = Math.round((e.files_inserted / e.files_total) * 100);
        rescanState = {
          ...rescanState!,
          progress: `Applying... ${e.files_inserted}/${e.files_total} (${pct}%)`,
        };
      }
    });

    try {
      const result = await invoke<RescanApplyResult>('apply_rescan', { folderId: folder.id });

      if (rescanUnlisten) {
        rescanUnlisten();
        rescanUnlisten = null;
      }

      rescanState = {
        phase: 'done',
        progress: '',
        preview: rescanState.preview,
        applyResult: result,
      };

      // Refresh everything
      await loadFolders();
      exploreState.clearCache();
      await exploreState.loadRoots(true);
      const currentType = viewState.activeTab === 'images' ? 'image' : 'audio';
      await assetsState.loadAssets(currentType);
      await processingState.refreshPendingCount();
      await emit('scan-complete');

      const parts = [];
      if (result.inserted > 0) parts.push(`${result.inserted} added`);
      if (result.deleted > 0) parts.push(`${result.deleted} removed`);
      if (result.updated > 0) parts.push(`${result.updated} updated`);
      showToast('Rescan applied: ' + parts.join(', '), 'success');

      setTimeout(dismissRescan, 4000);
    } catch (error) {
      showToast('Apply failed: ' + error, 'error');
      dismissRescan();
    }
  }

  function dismissRescan() {
    rescanFolderId = null;
    rescanState = null;
    if (rescanUnlisten) {
      rescanUnlisten();
      rescanUnlisten = null;
    }
  }

  function folderName(path: string): string {
    const parts = path.replace(/\\/g, '/').split('/');
    return parts[parts.length - 1] || path;
  }

  // Tick counter that updates every second while any scan is active, for elapsed time display
  let nowMs = $state(Date.now());
  $effect(() => {
    if (uiState.isScanning) {
      nowMs = Date.now();
      const interval = setInterval(() => {
        nowMs = Date.now();
      }, 1000);
      return () => clearInterval(interval);
    }
  });
</script>

<div class="flex flex-col h-full overflow-auto p-6">
  <!-- Header -->
  <div class="flex items-center justify-between mb-6">
    <div>
      <h1 class="text-2xl font-semibold text-primary">Sources</h1>
      <p class="text-sm text-tertiary mt-0.5">Manage source folders for asset scanning</p>
    </div>
    <button
      onclick={addFolder}
      class="flex items-center gap-2 px-4 py-2 text-sm font-medium rounded-lg bg-accent text-white hover:bg-accent/90 transition-colors"
    >
      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
      </svg>
      Add Folder
    </button>
  </div>

  <!-- Active scan progress cards -->
  {#each [...uiState.activeScans.values()] as scan (scan.folderPath)}
    {@const elapsed = nowMs - scan.startedAt}
    <div class="mb-4 rounded-lg border border-default bg-secondary p-4 flex items-center gap-3">
      <Spinner size="sm" />
      <div class="flex flex-col min-w-0">
        <div class="flex items-center gap-2">
          <span class="text-sm font-medium text-primary">{folderName(scan.folderPath)}</span>
          <span class="text-sm text-secondary">{scan.progressMessage}</span>
          {#if elapsed > 0}
            <span class="text-xs text-tertiary">{formatElapsed(elapsed)}</span>
          {/if}
        </div>
        {#if scan.details.currentPath}
          <span
            class="text-xs text-tertiary overflow-hidden whitespace-nowrap text-ellipsis block"
            style="direction: rtl;"
            title={scan.details.currentPath}
          >
            {scan.details.currentPath}
          </span>
        {/if}
      </div>
    </div>
  {/each}

  <!-- Folder list -->
  {#if loading}
    <div class="flex items-center justify-center py-16">
      <Spinner size="lg" />
    </div>
  {:else if folders.length === 0 && !uiState.isScanning}
    <div class="flex flex-col items-center justify-center py-16 text-center">
      <div class="w-16 h-16 rounded-full bg-tertiary flex items-center justify-center mb-4">
        <FolderIcon class="w-8 h-8 text-tertiary" />
      </div>
      <h3 class="text-lg font-medium text-primary mb-1">No folders yet</h3>
      <p class="text-sm text-tertiary mb-4">Add a folder to start scanning for assets</p>
      <button
        onclick={addFolder}
        class="px-4 py-2 text-sm font-medium rounded-lg bg-accent text-white hover:bg-accent/90 transition-colors"
      >
        Add Folder
      </button>
    </div>
  {:else}
    <div class="space-y-2">
      {#each folders.filter((f) => !uiState.isScanningFolder(f.path)) as folder (folder.id)}
        <div class="rounded-lg border border-default bg-secondary transition-colors">
          <div class="p-4">
            <div class="flex items-start justify-between gap-4">
              <!-- Folder info -->
              <div class="flex items-start gap-3 min-w-0 flex-1">
                <div
                  class="w-9 h-9 rounded-lg bg-accent/10 flex items-center justify-center flex-shrink-0 mt-0.5"
                >
                  <FolderIcon class="w-5 h-5 text-accent" />
                </div>
                <div class="min-w-0 flex-1">
                  <!-- Name / rename -->
                  {#if editingId === folder.id}
                    <input
                      bind:this={editInput}
                      bind:value={editLabel}
                      onblur={() => saveRename(folder)}
                      onkeydown={(e) => handleRenameKeydown(e, folder)}
                      class="w-full text-sm font-medium text-primary bg-primary border border-accent rounded px-2 py-1 outline-none"
                    />
                  {:else}
                    <button
                      onclick={() => startRename(folder)}
                      class="text-sm font-medium text-primary hover:text-accent transition-colors text-left truncate block max-w-full"
                      title="Click to rename"
                    >
                      {folder.label || folderName(folder.path)}
                    </button>
                  {/if}
                  <p class="text-xs text-tertiary truncate mt-0.5" title={folder.path}>
                    {folder.path}
                  </p>
                  <!-- Stats -->
                  <div class="flex items-center gap-4 mt-2 text-xs text-secondary">
                    <span>{formatCount(folder.asset_count)} assets</span>
                    <span class="text-tertiary">Scanned {formatDate(folder.last_scanned_at)}</span>
                  </div>
                </div>
              </div>

              <!-- Actions -->
              {#if removingId === folder.id}
                <div class="flex items-center gap-2 flex-shrink-0 text-xs text-secondary">
                  <Spinner size="sm" />
                  <span>{removeProgress}</span>
                </div>
              {:else}
                <div class="flex items-center gap-1 flex-shrink-0">
                  <button
                    onclick={() => openPath(folder.path)}
                    disabled={removingId !== null}
                    class="p-2 rounded-lg text-tertiary hover:text-primary hover:bg-tertiary transition-colors disabled:opacity-30 disabled:pointer-events-none"
                    title="Open in file explorer"
                  >
                    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        stroke-width="1.5"
                        d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14"
                      />
                    </svg>
                  </button>
                  <button
                    onclick={() => startRescan(folder)}
                    disabled={rescanFolderId !== null || uiState.isScanning || removingId !== null}
                    class="p-2 rounded-lg text-tertiary hover:text-accent hover:bg-accent/10 transition-colors disabled:opacity-30 disabled:pointer-events-none"
                    title="Rescan for changes"
                  >
                    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        stroke-width="1.5"
                        d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
                      />
                    </svg>
                  </button>
                  <button
                    onclick={() => startRename(folder)}
                    disabled={removingId !== null}
                    class="p-2 rounded-lg text-tertiary hover:text-primary hover:bg-tertiary transition-colors disabled:opacity-30 disabled:pointer-events-none"
                    title="Rename"
                  >
                    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        stroke-width="1.5"
                        d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z"
                      />
                    </svg>
                  </button>
                  <button
                    onclick={() => removeFolder(folder)}
                    disabled={removingId !== null}
                    class="p-2 rounded-lg text-tertiary hover:text-error hover:bg-error/10 transition-colors disabled:opacity-30 disabled:pointer-events-none"
                    title="Remove folder"
                  >
                    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        stroke-width="1.5"
                        d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
                      />
                    </svg>
                  </button>
                </div>
              {/if}
            </div>

            <!-- Scan warnings (persistent) -->
            {#if folder.scan_warnings}
              {@const warnings = JSON.parse(folder.scan_warnings) as string[]}
              <div class="mt-3 rounded-md bg-warning/10 border border-warning/20 px-3 py-2">
                <p class="text-xs font-medium text-warning mb-1">
                  {warnings.length} file{warnings.length !== 1 ? 's' : ''} could not be read during last scan
                </p>
                <ul class="space-y-0.5">
                  {#each warnings as w}
                    <li class="text-xs text-secondary font-mono truncate" title={w}>{w}</li>
                  {/each}
                </ul>
              </div>
            {/if}
          </div>

          <!-- Rescan panel (inline, below folder info) -->
          {#if rescanFolderId === folder.id && rescanState}
            <div class="border-t border-default px-4 py-3">
              {#if rescanState.phase === 'scanning'}
                <div class="flex items-center gap-3">
                  <Spinner size="sm" />
                  <span class="text-sm text-secondary">{rescanState.progress}</span>
                </div>
              {:else if rescanState.phase === 'preview' && rescanState.preview}
                {@const p = rescanState.preview}
                <div class="flex items-center justify-between gap-4">
                  <div class="flex items-center gap-4 text-sm">
                    {#if p.added_count > 0}
                      <span class="text-success font-medium">+{p.added_count} new</span>
                    {/if}
                    {#if p.removed_count > 0}
                      <span class="text-error font-medium">&minus;{p.removed_count} removed</span>
                    {/if}
                    {#if p.modified_count > 0}
                      <span class="text-warning font-medium">{p.modified_count} modified</span>
                    {/if}
                    <span class="text-tertiary">{p.unchanged_count.toLocaleString()} unchanged</span
                    >
                  </div>
                  <div class="flex items-center gap-2">
                    <button
                      onclick={dismissRescan}
                      class="px-3 py-1.5 text-sm text-secondary hover:text-primary hover:bg-tertiary rounded-lg transition-colors"
                    >
                      Cancel
                    </button>
                    <button
                      onclick={() => applyRescan(folder)}
                      class="px-3 py-1.5 text-sm font-medium text-white bg-accent hover:bg-accent/90 rounded-lg transition-colors"
                    >
                      Apply Changes
                    </button>
                  </div>
                </div>
              {:else if rescanState.phase === 'applying'}
                <div class="flex items-center gap-3">
                  <Spinner size="sm" />
                  <span class="text-sm text-secondary">{rescanState.progress}</span>
                </div>
              {:else if rescanState.phase === 'done' && rescanState.preview}
                <div class="flex items-center gap-2 text-sm text-success">
                  <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path
                      stroke-linecap="round"
                      stroke-linejoin="round"
                      stroke-width="2"
                      d="M5 13l4 4L19 7"
                    />
                  </svg>
                  {#if rescanState.applyResult}
                    Changes applied
                  {:else}
                    Up to date
                  {/if}
                </div>
              {/if}
            </div>
          {/if}

          <!-- Search depth settings -->
          <SearchConfigPanel folderId={folder.id} folderPath={folder.path} />
        </div>
      {/each}
    </div>
  {/if}
</div>
