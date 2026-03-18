<script lang="ts">
  import { clapState, type ClapSetupStatus } from '$lib/state/clap.svelte';
  import { showToast, showConfirm } from '$lib/state/ui.svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { openPath } from '@tauri-apps/plugin-opener';
  import ClapSetupDialog from '$lib/components/ClapSetupDialog.svelte';

  let showSetupDialog = $state(false);

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

  function formatBytes(bytes: number): string {
    if (bytes === 0) return '0 B';
    const units = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(1024));
    return `${(bytes / Math.pow(1024, i)).toFixed(i > 0 ? 1 : 0)} ${units[i]}`;
  }

  async function handleSetup() {
    showSetupDialog = true;
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

  function handleSetupComplete() {
    showSetupDialog = false;
    showToast('Semantic search is ready', 'success');
    clapState.refreshCacheSize();
  }

  function handleSetupCancel() {
    showSetupDialog = false;
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

<div class="h-full overflow-y-auto p-8">
  <div class="max-w-2xl">
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

        <!-- Setup / Info -->
        {#if clapState.setupStatus === 'not-configured' || clapState.setupStatus === 'error'}
          <div class="flex items-center justify-between pt-3 border-t border-default">
            <div class="text-sm text-secondary">
              {#if clapState.setupError}
                <span class="text-error">{clapState.setupError}</span>
              {:else}
                Set up to search audio files by describing what they sound like.
              {/if}
            </div>
            <button
              onclick={handleSetup}
              class="px-4 py-2 text-sm font-medium rounded-lg bg-accent text-white hover:bg-accent/90 transition-colors"
            >
              {clapState.setupStatus === 'error' ? 'Retry Setup' : 'Set Up'}
            </button>
          </div>
        {:else if clapState.setupStatus === 'ready'}
          <!-- Runtime Info -->
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
              <span class="text-secondary ml-2">{formatBytes(clapState.cacheSize)}</span>
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

    <!-- About Section -->
    <section>
      <h2 class="text-lg font-medium text-primary mb-4">About</h2>
      <div class="rounded-lg border border-default bg-secondary p-5">
        <div class="flex items-center justify-between text-sm">
          <span class="text-tertiary">Version</span>
          <span class="text-secondary">0.1.0</span>
        </div>
      </div>
    </section>
  </div>
</div>

{#if showSetupDialog}
  <ClapSetupDialog onComplete={handleSetupComplete} onCancel={handleSetupCancel} />
{/if}
