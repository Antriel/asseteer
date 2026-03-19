<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { onMount, onDestroy } from 'svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import { showToast, showConfirm, uiState } from '$lib/state/ui.svelte';
  import { exploreState } from '$lib/state/explore.svelte';
  import { assetsState } from '$lib/state/assets.svelte';
  import { processingState } from '$lib/state/tasks.svelte';
  import { viewState } from '$lib/state/view.svelte';
  import type { SourceFolder } from '$lib/types';
  import Spinner from '$lib/components/shared/Spinner.svelte';
  import FolderIcon from '$lib/components/icons/FolderIcon.svelte';

  interface ScanProgressEvent {
    phase: 'discovering' | 'inserting' | 'scanning' | 'complete';
    files_found: number;
    files_inserted: number;
    files_total: number;
    zips_scanned: number;
    current_path: string | null;
  }

  let folders = $state<SourceFolder[]>([]);
  let loading = $state(true);
  let editingId = $state<number | null>(null);
  let editLabel = $state('');
  let editInput = $state<HTMLInputElement | null>(null);
  let scanningFolderId = $state<number | null>(null);
  let scanProgress = $state('');
  let unlisten: UnlistenFn | null = null;

  onMount(async () => {
    await loadFolders();
  });

  onDestroy(() => {
    if (unlisten) {
      unlisten();
      unlisten = null;
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
    // Focus input after DOM update
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
      `Remove "${name}" and all ${folder.asset_count.toLocaleString()} assets? This cannot be undone.`,
      'Remove Folder',
      'Remove',
    );
    if (!confirmed) return;

    try {
      await invoke('remove_folder', { folderId: folder.id });
      folders = folders.filter((f) => f.id !== folder.id);
      showToast(`Removed "${name}"`, 'success');
      // Refresh explore tree and assets
      exploreState.clearCache();
      await exploreState.loadRoots(true);
      const currentType = viewState.activeTab === 'images' ? 'image' : 'audio';
      await assetsState.loadAssets(currentType);
    } catch (error) {
      showToast('Failed to remove folder: ' + error, 'error');
    }
  }

  // Add folder
  async function addFolder() {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select folder to add',
      });
      if (!selected || typeof selected !== 'string') return;

      scanningFolderId = -1; // Indicate new folder scan
      scanProgress = 'Starting scan...';
      uiState.isScanning = true;
      uiState.resetScanDetails();

      if (unlisten) {
        unlisten();
        unlisten = null;
      }

      unlisten = await listen<ScanProgressEvent>('scan-progress', (event) => {
        const e = event.payload;
        if (e.phase === 'discovering') {
          const zipInfo = e.zips_scanned > 0 ? ` (${e.zips_scanned} zips)` : '';
          scanProgress = `Discovering... ${e.files_found} found${zipInfo}`;
        } else if (e.phase === 'inserting' || e.phase === 'scanning') {
          if (e.files_total > 0) {
            const pct = Math.round((e.files_inserted / e.files_total) * 100);
            scanProgress = `Saving... ${e.files_inserted}/${e.files_total} (${pct}%)`;
          } else {
            scanProgress = `Scanning... ${e.files_found} found`;
          }
        } else {
          scanProgress = `Done! ${e.files_found} assets.`;
        }
      });

      await invoke('add_folder', { path: selected });
      showToast('Folder added successfully', 'success');
      await loadFolders();
      exploreState.clearCache();
      await exploreState.loadRoots(true);
      await processingState.refreshPendingCount();
    } catch (error) {
      showToast('Failed to add folder: ' + error, 'error');
    } finally {
      uiState.isScanning = false;
      scanningFolderId = null;
      scanProgress = '';
      if (unlisten) {
        unlisten();
        unlisten = null;
      }
    }
  }

  function folderName(path: string): string {
    const parts = path.replace(/\\/g, '/').split('/');
    return parts[parts.length - 1] || path;
  }
</script>

<div class="h-full overflow-y-auto p-8">
  <div class="max-w-3xl">
    <!-- Header -->
    <div class="flex items-center justify-between mb-6">
      <div>
        <h1 class="text-2xl font-semibold text-primary">Folders</h1>
        <p class="text-sm text-tertiary mt-0.5">Manage source folders for asset scanning</p>
      </div>
      <button
        onclick={addFolder}
        disabled={uiState.isScanning}
        class="flex items-center gap-2 px-4 py-2 text-sm font-medium rounded-lg bg-accent text-white hover:bg-accent/90 transition-colors disabled:opacity-50"
      >
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
        </svg>
        Add Folder
      </button>
    </div>

    <!-- Scanning indicator -->
    {#if uiState.isScanning && scanProgress}
      <div class="mb-4 rounded-lg border border-default bg-secondary p-4 flex items-center gap-3">
        <Spinner size="sm" />
        <span class="text-sm text-secondary">{scanProgress}</span>
      </div>
    {/if}

    <!-- Folder list -->
    {#if loading}
      <div class="flex items-center justify-center py-16">
        <Spinner size="lg" />
      </div>
    {:else if folders.length === 0}
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
        {#each folders as folder (folder.id)}
          <div class="rounded-lg border border-default bg-secondary hover:bg-tertiary/50 transition-colors">
            <div class="p-4">
              <div class="flex items-start justify-between gap-4">
                <!-- Folder info -->
                <div class="flex items-start gap-3 min-w-0 flex-1">
                  <div class="w-9 h-9 rounded-lg bg-accent/10 flex items-center justify-center flex-shrink-0 mt-0.5">
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
                <div class="flex items-center gap-1 flex-shrink-0">
                  <button
                    onclick={() => startRename(folder)}
                    class="p-2 rounded-lg text-tertiary hover:text-primary hover:bg-tertiary transition-colors"
                    title="Rename"
                  >
                    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
                    </svg>
                  </button>
                  <button
                    onclick={() => removeFolder(folder)}
                    class="p-2 rounded-lg text-tertiary hover:text-error hover:bg-error/10 transition-colors"
                    title="Remove folder"
                  >
                    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                    </svg>
                  </button>
                </div>
              </div>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>
