<script lang="ts">
  import type { ProcessingCategory, CategoryProgress } from '$lib/types';
  import {
    processingState,
    getCategoryStatus,
    formatElapsed,
    isCategoryStarting,
    isCategoryQueued,
  } from '$lib/state/tasks.svelte';
  import { clapState } from '$lib/state/clap.svelte';
  import { settings } from '$lib/state/settings.svelte';
  import { showToast } from '$lib/state/ui.svelte';
  import { goto } from '$app/navigation';
  import Spinner from './shared/Spinner.svelte';
  import ProcessingDetails from './ProcessingDetails.svelte';

  interface Props {
    category: ProcessingCategory;
  }

  let { category }: Props = $props();

  // Derive progress and pending count from processing state
  let progress = $derived(processingState.categoryProgress.get(category) ?? null);
  let pendingCount = $derived(processingState.getPendingCountForCategory(category));

  // Compute display values
  let status = $derived(getCategoryStatus(processingState, category));
  let total = $derived(progress?.total || 0);
  let completed = $derived(progress?.completed || 0);
  let failed = $derived(progress?.failed || 0);
  let processed = $derived(completed + failed);
  let ratio = $derived(total === 0 ? 0 : processed / total);
  let percentage = $derived(Math.floor(ratio * 100));
  let durationMs = $derived(processingState.categoryDurationMs.get(category));
  let isStarting = $derived(isCategoryStarting(processingState, category));
  let isQueued = $derived(isCategoryQueued(processingState, category));
  let isStopping = $derived(processingState.stoppingCategories.has(category));
  let disabled = $derived(pendingCount === 0 && !progress?.isRunning);

  // Category config
  const categoryConfig: Record<ProcessingCategory, {
    label: string;
    description: string;
    icon: string;
    iconColor: string;
    accentColor: string;
  }> = {
    audio: {
      label: 'Audio',
      description: 'Extracts duration, sample rate, and channel info',
      icon: 'M9 19V6l12-3v13M9 19c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zm12-3c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zM9 10l12-3',
      iconColor: 'text-emerald-600 dark:text-emerald-400 bg-emerald-100 dark:bg-emerald-900/30',
      accentColor: 'bg-blue-500',
    },
    image: {
      label: 'Image',
      description: 'Generates thumbnails and extracts dimensions',
      icon: 'M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z',
      iconColor: 'text-blue-600 dark:text-blue-400 bg-blue-100 dark:bg-blue-900/30',
      accentColor: 'bg-blue-500',
    },
    clap: {
      label: 'CLAP Embeddings',
      description: 'Enables semantic audio search',
      icon: 'M9 19V6l12-3v13M9 19c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zm12-3c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zM9 10l12-3',
      iconColor: 'text-purple-600 dark:text-purple-400 bg-purple-100 dark:bg-purple-900/30',
      accentColor: 'bg-purple-500',
    },
  };

  let config = $derived(categoryConfig[category]);
  let isClap = $derived(category === 'clap');

  // CLAP server status badge — only shown for server-level states
  let serverBadge = $derived.by(() => {
    if (!isClap) return null;
    if (!clapState.serverAvailable && !clapState.serverStarting) {
      return {
        label: 'Server Offline',
        bgClass: 'bg-gray-100 dark:bg-gray-700',
        textClass: 'text-gray-700 dark:text-gray-300',
      };
    }
    if (clapState.serverStarting) {
      return {
        label: 'Server Starting',
        bgClass: 'bg-purple-100 dark:bg-purple-900/30',
        textClass: 'text-purple-700 dark:text-purple-300',
      };
    }
    return null;
  });

  // Button visibility
  let canStart = $derived(
    status === 'idle' &&
      !disabled &&
      !isStarting &&
      !isQueued &&
      (!isClap || clapState.serverAvailable),
  );
  let canPause = $derived(status === 'running' && !isStopping && !isStarting);
  let canResume = $derived(status === 'paused' && !isStopping && !isStarting);
  let canStop = $derived(
    (status === 'running' || status === 'paused' || isQueued) && !isStopping && !isStarting,
  );

  // Event handlers
  async function handleStart() {
    try {
      if (isClap && !clapState.serverAvailable) {
        const success = await clapState.ensureServer();
        if (!success) {
          showToast('Cannot start: CLAP server unavailable', 'error');
          return;
        }
      }
      await processingState.startProcessing(category);
    } catch (error) {
      if (isClap) {
        showToast(`Failed to start: ${error}`, 'error');
      } else {
        console.error(`Failed to start ${category}:`, error);
      }
    }
  }

  async function handlePause() {
    try {
      await processingState.pause(category);
    } catch (error) {
      if (isClap) {
        showToast(`Failed to pause: ${error}`, 'error');
      } else {
        console.error(`Failed to pause ${category}:`, error);
      }
    }
  }

  async function handleResume() {
    try {
      await processingState.resume(category);
    } catch (error) {
      if (isClap) {
        showToast(`Failed to resume: ${error}`, 'error');
      } else {
        console.error(`Failed to resume ${category}:`, error);
      }
    }
  }

  async function handleStop() {
    try {
      await processingState.stop(category);
    } catch (error) {
      if (isClap) {
        showToast(`Failed to stop: ${error}`, 'error');
      } else {
        console.error(`Failed to stop ${category}:`, error);
      }
    }
  }

  async function handleStartServer() {
    try {
      const success = await clapState.ensureServer();
      if (success) {
        showToast('CLAP server started', 'success');
        await processingState.refreshPendingCount();
      } else {
        showToast('Failed to start CLAP server', 'error');
      }
    } catch (error) {
      showToast(`Failed to start server: ${error}`, 'error');
    }
  }
</script>

<div
  class="flex flex-col gap-3 p-4 bg-primary border border-default rounded-lg transition-opacity"
  class:opacity-50={disabled && !isClap}
>
  <!-- Header: Category name and controls -->
  <div class="flex items-center justify-between">
    <div class="flex items-center gap-3">
      <div class="w-8 h-8 flex items-center justify-center rounded {config.iconColor}">
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d={config.icon} />
        </svg>
      </div>
      <div>
        <h4 class="text-base font-semibold text-primary">{config.label}</h4>
        <p class="text-xs text-secondary">
          {config.description}
          {#if status === 'completed' && durationMs}
            <span class="text-tertiary">&middot; completed in {formatElapsed(durationMs)}</span>
          {/if}
        </p>
      </div>
    </div>

    <div class="flex items-center gap-2">
      <!-- Server status badge (CLAP only) -->
      {#if serverBadge}
        <span class="text-xs px-2 py-1 rounded {serverBadge.bgClass} {serverBadge.textClass}">
          {serverBadge.label}
        </span>
      {/if}

      <!-- Control buttons -->
      {#if isQueued}
        <span
          class="px-3 py-1.5 text-sm font-medium text-indigo-700 dark:text-indigo-300 bg-indigo-100 dark:bg-indigo-900/30 rounded"
        >
          Queued
        </span>
      {:else if isStarting}
        <span
          class="px-3 py-1.5 text-sm font-medium text-blue-700 dark:text-blue-300 bg-blue-100 dark:bg-blue-900/30 rounded"
        >
          Starting...
        </span>
      {:else if canStart}
        <button
          onclick={handleStart}
          class="px-3 py-1.5 text-sm font-medium text-white {isClap
            ? 'bg-purple-500 hover:bg-purple-600'
            : 'bg-blue-500 hover:bg-blue-600'} rounded transition-colors"
        >
          Start
        </button>
      {/if}

      {#if canPause}
        <button
          onclick={handlePause}
          class="px-3 py-1.5 text-sm font-medium text-white bg-orange-500 hover:bg-orange-600 rounded transition-colors"
        >
          Pause
        </button>
      {/if}

      {#if canResume}
        <button
          onclick={handleResume}
          class="px-3 py-1.5 text-sm font-medium text-white bg-green-500 hover:bg-green-600 rounded transition-colors"
        >
          Resume
        </button>
      {/if}

      {#if isStopping}
        <span
          class="px-3 py-1.5 text-sm font-medium text-yellow-700 dark:text-yellow-300 bg-yellow-100 dark:bg-yellow-900/30 rounded"
        >
          Stopping...
        </span>
      {:else if canStop}
        <button
          onclick={handleStop}
          class="px-3 py-1.5 text-sm font-medium text-white bg-red-500 hover:bg-red-600 rounded transition-colors"
        >
          Stop
        </button>
      {/if}
    </div>
  </div>

  <!-- CLAP server status section -->
  {#if isClap && !clapState.serverAvailable && !clapState.serverStarting}
    <div class="flex items-center justify-between p-3 bg-secondary rounded">
      {#if clapState.setupStatus === 'not-configured'}
        <span class="text-sm text-secondary">Semantic search requires one-time setup</span>
        <button
          onclick={() => goto('/settings')}
          class="px-3 py-1.5 text-sm font-medium text-white bg-purple-500 hover:bg-purple-600 rounded transition-colors"
        >
          Go to Settings
        </button>
      {:else}
        <span class="text-sm text-secondary">CLAP server not running</span>
        <button
          onclick={handleStartServer}
          class="px-3 py-1.5 text-sm font-medium text-white bg-purple-500 hover:bg-purple-600 rounded transition-colors"
        >
          Start Server
        </button>
      {/if}
    </div>
  {:else if isClap && clapState.serverStarting}
    <div class="flex items-center gap-2 p-3 bg-secondary rounded">
      <Spinner size="sm" />
      <span class="text-sm text-secondary">
        {#if clapState.startupPhase === 'downloading-uv'}
          Downloading runtime tools...
        {:else if clapState.startupPhase === 'loading-model'}
          Loading AI model...
        {:else if clapState.startupDetail}
          {clapState.startupDetail}...
        {:else}
          Starting CLAP server...
        {/if}
      </span>
    </div>
  {:else}
    <!-- Progress info -->
    {#if isQueued && pendingCount > 0}
      <div class="flex items-center gap-2 text-sm">
        <span class="text-secondary">Waiting to start:</span>
        <span class="font-semibold text-primary">{pendingCount} assets</span>
      </div>
    {:else if status === 'idle' && pendingCount > 0 && !isStarting}
      <div class="flex items-center gap-2 text-sm">
        {#if isClap}
          <span class="text-orange-600 dark:text-orange-400 font-medium">
            {pendingCount} audio files
          </span>
          <span class="text-secondary">need embeddings</span>
        {:else}
          <span class="text-secondary">Pending:</span>
          <span class="font-semibold text-primary">{pendingCount} assets</span>
        {/if}
      </div>
      {#if category === 'image'}
        <div class="flex items-center justify-between pt-2 border-t border-default">
          <div>
            <div class="text-sm text-primary">Pre-generate thumbnails</div>
            <div class="text-xs text-tertiary mt-0.5">
              Generate thumbnails during processing instead of on scroll. Slower processing, faster
              browsing.
            </div>
          </div>
          <button
            role="switch"
            aria-label="Pre-generate thumbnails"
            aria-checked={settings.preGenerateThumbnails}
            onclick={() => settings.setPreGenerateThumbnails(!settings.preGenerateThumbnails)}
            class="relative inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full transition-colors
              {settings.preGenerateThumbnails ? 'bg-accent' : 'bg-tertiary'}"
          >
            <span
              class="inline-block h-3.5 w-3.5 rounded-full bg-white shadow transition-transform
                {settings.preGenerateThumbnails ? 'translate-x-4' : 'translate-x-1'}"
            ></span>
          </button>
        </div>
      {/if}
    {:else if status === 'idle' && pendingCount === 0 && !isStarting}
      <div class="text-sm text-secondary">
        {#if isClap}
          <span class="text-green-600 dark:text-green-400">All audio files have embeddings</span>
        {:else}
          No assets to process
        {/if}
      </div>
    {:else if progress}
      <!-- Processing state: show progress bar and stats -->
      <div class="flex flex-col gap-2">
        <div class="flex flex-col gap-1">
          <div class="flex items-center justify-between text-xs">
            <span class="text-secondary">Progress</span>
            <span class="font-medium text-primary">
              {processed} / {total} ({percentage}%)
            </span>
          </div>
          <div class="w-full h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
            <div
              class="h-full {config.accentColor} transition-all duration-300"
              style="width: {ratio * 100}%"
            ></div>
          </div>
        </div>

        <!-- Stats: completed and failed -->
        <div class="flex items-center gap-4 text-sm">
          <div class="flex items-center gap-1.5">
            <svg
              class="w-4 h-4 text-green-500"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M5 13l4 4L19 7"
              />
            </svg>
            <span class="text-secondary">Completed:</span>
            <span class="font-semibold text-green-600 dark:text-green-400">{completed}</span>
          </div>

          {#if failed > 0}
            <div class="flex items-center gap-1.5">
              <svg
                class="w-4 h-4 text-red-500"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
              >
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M6 18L18 6M6 6l12 12"
                />
              </svg>
              <span class="text-secondary">Failed:</span>
              <span class="font-semibold text-red-600 dark:text-red-400">{failed}</span>
            </div>
          {/if}
        </div>

        <ProcessingDetails {category} {progress} />
      </div>
    {/if}
  {/if}
</div>
