<script lang="ts">
  import { clapState, type ClapSetupStatus, type ClapStartupPhase } from '$lib/state/clap.svelte';
  import { checkClapSetupState } from '$lib/database/queries';
  import { showToast, showConfirm } from '$lib/state/ui.svelte';
  import { formatFileSize } from '$lib/utils/format';
  import { invoke } from '@tauri-apps/api/core';
  import { openPath } from '@tauri-apps/plugin-opener';

  let isFirstTimeSetup = $state(false);
  let showDownloadStep = $state(false);

  // Database info
  interface DbInfo {
    path: string;
    main_size: number;
    wal_size: number;
    page_count: number;
    page_size: number;
    freelist_count: number;
    total_assets: number;
    total_folders: number;
  }
  let dbInfo = $state<DbInfo | null>(null);
  let dbLoading = $state(false);
  let vacuumRunning = $state(false);

  async function loadDbInfo() {
    dbLoading = true;
    try {
      dbInfo = await invoke<DbInfo>('get_db_info');
    } catch (error) {
      showToast('Failed to load database info: ' + error, 'error');
    } finally {
      dbLoading = false;
    }
  }

  async function handleOpenDbFolder() {
    if (!dbInfo) return;
    try {
      const folder = dbInfo.path.replace(/[\\/][^\\/]+$/, '');
      await openPath(folder);
    } catch (error) {
      showToast('Failed to open folder: ' + error, 'error');
    }
  }

  async function handleVacuum() {
    const confirmed = await showConfirm(
      'This will compact the database to reclaim unused space. The app may be unresponsive briefly during this operation.',
      'Compact Database',
      'Compact',
    );
    if (!confirmed) return;

    vacuumRunning = true;
    try {
      await invoke('vacuum_database');
      showToast('Database compacted successfully', 'success');
      await loadDbInfo();
    } catch (error) {
      showToast('Failed to compact database: ' + error, 'error');
    } finally {
      vacuumRunning = false;
    }
  }

  // Load DB info on mount
  loadDbInfo();

  const statusLabel: Record<ClapSetupStatus, string> = {
    'not-configured': 'Not set up',
    'setting-up': 'Setting up...',
    ready: 'Ready',
    offline: 'Offline',
    error: 'Error',
  };

  const statusColor: Record<ClapSetupStatus, string> = {
    'not-configured': 'bg-tertiary',
    'setting-up': 'bg-warning',
    ready: 'bg-success',
    offline: 'bg-tertiary',
    error: 'bg-error',
  };

  const phaseOrder: ClapStartupPhase[] = [
    'downloading-uv',
    'starting-process',
    'waiting-for-server',
    'loading-model',
    'ready',
  ];

  type StepDisplayStatus = 'idle' | 'running' | 'done' | 'error';

  let steps = $derived.by(() => {
    const items: { key: string; label: string; hint: string }[] = [];
    if (showDownloadStep) {
      items.push({ key: 'downloading-uv', label: 'Downloading package manager', hint: '~30 MB' });
    }
    items.push(
      {
        key: 'starting-process',
        label: 'Setting up Python environment',
        hint: isFirstTimeSetup ? 'first run downloads ~3–8 GB' : '',
      },
      { key: 'loading-model', label: 'Loading model', hint: '' },
    );
    return items;
  });

  function getPhaseIndex(phase: ClapStartupPhase | null): number {
    if (!phase) return -1;
    const mapped = phase === 'waiting-for-server' ? 'starting-process' : phase;
    return phaseOrder.indexOf(mapped);
  }

  function stepStatus(stepKey: string): StepDisplayStatus {
    const currentPhase = clapState.startupPhase;
    const currentIdx = getPhaseIndex(currentPhase);
    const stepIdx = phaseOrder.indexOf(stepKey as ClapStartupPhase);

    const effectiveCurrentKey =
      currentPhase === 'waiting-for-server' ? 'starting-process' : currentPhase;

    if (stepKey === effectiveCurrentKey) return 'running';
    if (stepIdx < currentIdx) return 'done';
    return 'idle';
  }

  async function handleSetup() {
    try {
      const state = await checkClapSetupState();
      showDownloadStep = !state.uv_installed;
      isFirstTimeSetup = !state.cache_exists;
    } catch {
      showDownloadStep = true;
      isFirstTimeSetup = true;
    }

    if (isFirstTimeSetup) {
      const confirmed = await showConfirm(
        'Setting up semantic search requires downloading Python and the AI model (~3–8 GB depending on your GPU). This is a one-time download — future starts will be instant.',
        'Set Up Semantic Search',
        'Download & Set Up',
      );
      if (!confirmed) return;
    }

    clapState.setup().then((ok) => {
      if (ok) {
        showToast('Semantic search is ready', 'success');
        clapState.refreshCacheSize();
      }
    });
  }

  async function handleClearCache() {
    const confirmed = await showConfirm(
      'This will remove the downloaded Python environment and AI model. You will need to set up again to use semantic search.',
      'Clear Cache',
      'Clear',
    );
    if (!confirmed) return;

    try {
      await clapState.clearCache();
      showToast('Cache cleared successfully', 'success');
    } catch (error) {
      showToast('Failed to clear cache: ' + error, 'error');
    }
  }

  async function handleViewLogs() {
    try {
      const logDir = await invoke<string>('get_clap_log_dir');
      await openPath(logDir);
    } catch (error) {
      showToast('Failed to open log directory: ' + error, 'error');
    }
  }
</script>

<div class="flex flex-col h-full overflow-auto p-6">
  <div class="w-full max-w-3xl mx-auto">
    <h1 class="text-2xl font-semibold text-primary mb-1">Settings</h1>
    <p class="text-sm text-tertiary mb-8">Configure Asseteer features</p>

    <!-- Semantic Search Section -->
    <section class="mb-8">
      <h2 class="text-lg font-medium text-primary mb-4">Semantic Search</h2>
      <div class="rounded-lg border border-default bg-secondary p-5 space-y-5">
        <!-- Status Row -->
        <div class="flex items-center justify-between">
          <div>
            <div class="text-sm font-medium text-primary">CLAP Server</div>
            <div class="text-xs text-tertiary mt-0.5">
              AI-powered audio search using text descriptions
            </div>
          </div>
          <div class="flex items-center gap-2">
            <div class="w-2 h-2 rounded-full {statusColor[clapState.setupStatus]}"></div>
            <span class="text-sm text-secondary">{statusLabel[clapState.setupStatus]}</span>
          </div>
        </div>

        <!-- Not configured / Error -->
        {#if clapState.setupStatus === 'not-configured' || clapState.setupStatus === 'error'}
          <div class="flex items-center justify-between pt-3 border-t border-default gap-4">
            <div class="text-sm text-secondary">
              {#if clapState.setupError}
                <span class="text-error">{clapState.setupError}</span>
              {:else}
                Set up to search audio files by describing what they sound like.
              {/if}
            </div>
            <button
              onclick={handleSetup}
              class="shrink-0 px-4 py-2 text-sm font-medium rounded-lg bg-accent text-white hover:bg-accent/90 transition-colors"
            >
              {clapState.setupStatus === 'error' ? 'Retry' : 'Set Up'}
            </button>
          </div>

          <!-- Setting up: inline progress -->
        {:else if clapState.setupStatus === 'setting-up'}
          <div class="pt-3 border-t border-default space-y-3">
            {#if isFirstTimeSetup}
              <p class="text-xs text-tertiary">
                First-time setup downloads Python and the AI model. Keep this app open — closing it
                will cancel the download.
              </p>
            {/if}
            {#each steps as item (item.key)}
              {@const status = stepStatus(item.key)}
              <div class="flex items-center gap-3">
                <div class="w-5 h-5 flex items-center justify-center shrink-0">
                  {#if status === 'done'}
                    <svg
                      class="w-5 h-5 text-success"
                      fill="none"
                      stroke="currentColor"
                      viewBox="0 0 24 24"
                    >
                      <path
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        stroke-width="2"
                        d="M5 13l4 4L19 7"
                      />
                    </svg>
                  {:else if status === 'running'}
                    <div
                      class="w-4 h-4 border-2 border-accent border-t-transparent rounded-full animate-spin"
                    ></div>
                  {:else if status === 'error'}
                    <svg
                      class="w-5 h-5 text-error"
                      fill="none"
                      stroke="currentColor"
                      viewBox="0 0 24 24"
                    >
                      <path
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        stroke-width="2"
                        d="M6 18L18 6M6 6l12 12"
                      />
                    </svg>
                  {:else}
                    <div class="w-4 h-4 rounded-full border-2 border-default"></div>
                  {/if}
                </div>
                <span class="text-sm flex-1 {status === 'idle' ? 'text-tertiary' : 'text-primary'}">
                  {item.label}
                </span>
                {#if item.hint}
                  <span class="text-xs text-tertiary">{item.hint}</span>
                {/if}
              </div>
            {/each}
            {#if clapState.startupDetail}
              <div class="rounded bg-primary px-2 py-1.5">
                <p class="text-xs text-tertiary font-mono truncate">{clapState.startupDetail}</p>
              </div>
            {/if}
          </div>

          <!-- Ready: device/model info -->
        {:else if clapState.setupStatus === 'ready'}
          <div class="pt-3 border-t border-default space-y-2">
            <div class="flex items-center justify-between text-sm">
              <span class="text-tertiary">Device</span>
              <span class="text-secondary"
                >{clapState.device === 'cuda' ? 'GPU (CUDA)' : 'CPU'}</span
              >
            </div>
            <div class="flex items-center justify-between text-sm">
              <span class="text-tertiary">Model</span>
              <span class="text-secondary font-mono text-xs">{clapState.model ?? 'Unknown'}</span>
            </div>
          </div>

          <!-- Offline: restart button -->
        {:else if clapState.setupStatus === 'offline'}
          <div class="flex items-center justify-between pt-3 border-t border-default">
            <span class="text-sm text-secondary">CLAP server not running</span>
            <button
              onclick={handleSetup}
              class="px-4 py-2 text-sm font-medium rounded-lg bg-accent text-white hover:bg-accent/90 transition-colors"
            >
              Start Server
            </button>
          </div>
        {/if}

        <!-- Cache & Logs -->
        {#if clapState.cacheSize > 0}
          <div class="flex items-center justify-between pt-3 border-t border-default">
            <div class="text-sm">
              <span class="text-tertiary">Cache size</span>
              <span class="text-secondary ml-2">{formatFileSize(clapState.cacheSize)}</span>
            </div>
            <div class="flex items-center gap-2">
              <button
                onclick={handleViewLogs}
                class="px-3 py-1.5 text-xs font-medium rounded-lg border border-default text-secondary hover:text-primary hover:bg-tertiary transition-colors"
              >
                View Logs
              </button>
              <button
                onclick={handleClearCache}
                class="px-3 py-1.5 text-xs font-medium rounded-lg border border-default text-secondary hover:text-primary hover:bg-tertiary transition-colors"
              >
                Clear Cache
              </button>
            </div>
          </div>
        {/if}
      </div>
    </section>

    <!-- Database Section -->
    <section class="mb-8">
      <h2 class="text-lg font-medium text-primary mb-4">Database</h2>
      <div class="rounded-lg border border-default bg-secondary p-5 space-y-4">
        {#if dbLoading && !dbInfo}
          <div class="text-sm text-tertiary">Loading...</div>
        {:else if dbInfo}
          <!-- Size -->
          <div class="flex items-center justify-between text-sm">
            <span class="text-tertiary">Size</span>
            <span class="text-secondary">
              {formatFileSize(dbInfo.main_size + dbInfo.wal_size)}
              {#if dbInfo.wal_size > 0}
                <span class="text-tertiary ml-1">
                  ({formatFileSize(dbInfo.main_size)} + {formatFileSize(dbInfo.wal_size)} WAL)
                </span>
              {/if}
            </span>
          </div>

          <!-- Stats -->
          <div class="flex items-center justify-between text-sm">
            <span class="text-tertiary">Assets</span>
            <span class="text-secondary">{dbInfo.total_assets.toLocaleString()}</span>
          </div>
          <div class="flex items-center justify-between text-sm">
            <span class="text-tertiary">Source folders</span>
            <span class="text-secondary">{dbInfo.total_folders}</span>
          </div>
          {#if dbInfo.freelist_count > 0}
            <div class="flex items-center justify-between text-sm">
              <span class="text-tertiary">Reclaimable space</span>
              <span class="text-secondary">
                {formatFileSize(dbInfo.freelist_count * dbInfo.page_size)}
              </span>
            </div>
          {/if}

          <!-- Actions -->
          <div class="flex items-center justify-between pt-3 border-t border-default">
            <div class="flex items-center gap-2">
              <button
                onclick={handleOpenDbFolder}
                class="px-3 py-1.5 text-xs font-medium rounded-lg border border-default text-secondary hover:text-primary hover:bg-tertiary transition-colors"
              >
                Open DB Folder
              </button>
              <button
                onclick={handleVacuum}
                disabled={vacuumRunning}
                class="px-3 py-1.5 text-xs font-medium rounded-lg border border-default text-secondary hover:text-primary hover:bg-tertiary transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {#if vacuumRunning}
                  <span class="inline-flex items-center gap-1.5">
                    <span
                      class="w-3 h-3 border-2 border-current border-t-transparent rounded-full animate-spin"
                    ></span>
                    Compacting...
                  </span>
                {:else}
                  Compact Database
                {/if}
              </button>
            </div>
          </div>
        {/if}
      </div>
    </section>

    <!-- About Section -->
    <section>
      <h2 class="text-lg font-medium text-primary mb-4">About</h2>
      <div class="rounded-lg border border-default bg-secondary p-5">
        <div class="flex items-center justify-between text-sm">
          <span class="text-tertiary">Version</span>
          <span class="text-secondary">{__APP_VERSION__}</span>
        </div>
      </div>
    </section>
  </div>
</div>
