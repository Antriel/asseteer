<script lang="ts">
  import { clapState } from '$lib/state/clap.svelte';
  import {
    processingState,
    getCategoryStatus,
    isCategoryStarting,
  } from '$lib/state/tasks.svelte';
  import { showToast } from '$lib/state/ui.svelte';
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';
  import Spinner from './shared/Spinner.svelte';
  import ProcessingDetails from './ProcessingDetails.svelte';

  // Check server on mount
  onMount(async () => {
    await clapState.checkServer();
  });

  // Get progress from unified processing state
  let progress = $derived(processingState.categoryProgress.get('clap'));
  let pendingCount = $derived(processingState.pendingCount.clap);
  let status = $derived(getCategoryStatus(processingState, 'clap'));

  // Progress values
  let total = $derived(progress?.total || 0);
  let completed = $derived(progress?.completed || 0);
  let failed = $derived(progress?.failed || 0);
  let processed = $derived(completed + failed);
  let ratio = $derived(total === 0 ? 0 : processed / total);
  let percentage = $derived(Math.floor(ratio * 100));

  // Status configuration
  let statusConfig = $derived.by(() => {
    if (!clapState.serverAvailable && !clapState.serverStarting) {
      return {
        label: 'Server Offline',
        bgClass: 'bg-gray-100 dark:bg-gray-700',
        textClass: 'text-gray-700 dark:text-gray-300',
      };
    }
    switch (status) {
      case 'running':
        if (isStopping) {
          return {
            label: 'Stopping...',
            bgClass: 'bg-yellow-100 dark:bg-yellow-900/30',
            textClass: 'text-yellow-700 dark:text-yellow-300',
          };
        }
        return {
          label: 'Running',
          bgClass: 'bg-blue-100 dark:bg-blue-900/30',
          textClass: 'text-blue-700 dark:text-blue-300',
        };
      case 'paused':
        if (isStopping) {
          return {
            label: 'Stopping...',
            bgClass: 'bg-yellow-100 dark:bg-yellow-900/30',
            textClass: 'text-yellow-700 dark:text-yellow-300',
          };
        }
        return {
          label: 'Paused',
          bgClass: 'bg-orange-100 dark:bg-orange-900/30',
          textClass: 'text-orange-700 dark:text-orange-300',
        };
      case 'completed':
        if (failed > 0) {
          return {
            label: 'Completed with errors',
            bgClass: 'bg-red-100 dark:bg-red-900/30',
            textClass: 'text-red-700 dark:text-red-300',
          };
        }
        return {
          label: 'Completed',
          bgClass: 'bg-green-100 dark:bg-green-900/30',
          textClass: 'text-green-700 dark:text-green-300',
        };
      default:
        if (isStarting) {
          return {
            label: 'Starting...',
            bgClass: 'bg-blue-100 dark:bg-blue-900/30',
            textClass: 'text-blue-700 dark:text-blue-300',
          };
        }
        if (pendingCount === 0) {
          return {
            label: 'Ready',
            bgClass: 'bg-green-100 dark:bg-green-900/30',
            textClass: 'text-green-700 dark:text-green-300',
          };
        }
        return {
          label: 'Idle',
          bgClass: 'bg-gray-100 dark:bg-gray-700',
          textClass: 'text-gray-700 dark:text-gray-300',
        };
    }
  });

  // Stopping wind-down state
  let isStopping = $derived(processingState.stoppingCategories.has('clap'));
  let isStarting = $derived(isCategoryStarting(processingState, 'clap'));

  // Button visibility
  let canStart = $derived(clapState.serverAvailable && status === 'idle' && pendingCount > 0 && !isStarting);
  let canPause = $derived(status === 'running' && !isStopping && !isStarting);
  let canResume = $derived(status === 'paused' && !isStopping && !isStarting);
  let canStop = $derived((status === 'running' || status === 'paused') && !isStopping && !isStarting);

  async function handleStartServer() {
    try {
      const success = await clapState.ensureServer();
      if (success) {
        showToast('CLAP server started', 'success');
        // Refresh pending count now that server is available
        await processingState.refreshPendingCount();
      } else {
        showToast('Failed to start CLAP server', 'error');
      }
    } catch (error) {
      showToast(`Failed to start server: ${error}`, 'error');
    }
  }

  async function handleStart() {
    try {
      // Ensure server is running before starting processing
      if (!clapState.serverAvailable) {
        const success = await clapState.ensureServer();
        if (!success) {
          showToast('Cannot start: CLAP server unavailable', 'error');
          return;
        }
      }
      await processingState.startProcessing('clap');
    } catch (error) {
      showToast(`Failed to start: ${error}`, 'error');
    }
  }

  async function handlePause() {
    try {
      await processingState.pause('clap');
    } catch (error) {
      showToast(`Failed to pause: ${error}`, 'error');
    }
  }

  async function handleResume() {
    try {
      await processingState.resume('clap');
    } catch (error) {
      showToast(`Failed to resume: ${error}`, 'error');
    }
  }

  async function handleStop() {
    try {
      await processingState.stop('clap');
    } catch (error) {
      showToast(`Failed to stop: ${error}`, 'error');
    }
  }
</script>

<div class="flex flex-col gap-3 p-4 bg-primary border border-default rounded-lg">
  <!-- Header -->
  <div class="flex items-center justify-between">
    <div class="flex items-center gap-3">
      <!-- Audio wave icon -->
      <div
        class="w-8 h-8 flex items-center justify-center bg-purple-100 dark:bg-purple-900/30 rounded"
      >
        <svg
          class="w-5 h-5 text-purple-600 dark:text-purple-400"
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
        >
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M9 19V6l12-3v13M9 19c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zm12-3c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zM9 10l12-3"
          />
        </svg>
      </div>
      <div>
        <h4 class="text-base font-semibold text-primary">CLAP Embeddings</h4>
        <p class="text-xs text-secondary">Enables semantic audio search</p>
      </div>
    </div>

    <!-- Status badge -->
    <span class="text-xs px-2 py-1 rounded {statusConfig.bgClass} {statusConfig.textClass}">
      {statusConfig.label}
    </span>
  </div>

  <!-- Server status and controls -->
  {#if !clapState.serverAvailable && !clapState.serverStarting}
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
  {:else if clapState.serverStarting}
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
    <!-- Server is available - show processing controls -->
    <div class="flex items-center justify-between">
      <div class="text-sm">
        {#if status === 'idle' && pendingCount > 0 && !isStarting}
          <span class="text-orange-600 dark:text-orange-400 font-medium">
            {pendingCount} audio files
          </span>
          <span class="text-secondary"> need embeddings</span>
        {:else if status === 'idle' && pendingCount === 0 && !isStarting}
          <span class="text-green-600 dark:text-green-400">All audio files have embeddings</span>
        {/if}
      </div>

      <!-- Control buttons -->
      <div class="flex items-center gap-2">
        {#if isStarting}
          <span class="px-3 py-1.5 text-sm font-medium text-blue-700 dark:text-blue-300 bg-blue-100 dark:bg-blue-900/30 rounded">
            Starting...
          </span>
        {:else if canStart}
          <button
            onclick={handleStart}
            class="px-3 py-1.5 text-sm font-medium text-white bg-purple-500 hover:bg-purple-600 rounded transition-colors"
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

    <!-- Progress display when processing or completed -->
    {#if progress && (status === 'running' || status === 'paused' || status === 'completed' || isStarting)}
      <div class="flex flex-col gap-2">
        <!-- Progress bar -->
        <div class="flex flex-col gap-1">
          <div class="flex items-center justify-between text-xs">
            <span class="text-secondary">Progress</span>
            <span class="font-medium text-primary">
              {processed} / {total} ({percentage}%)
            </span>
          </div>
          <div class="w-full h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
            <div
              class="h-full bg-purple-500 transition-all duration-300"
              style="width: {ratio * 100}%"
            ></div>
          </div>
        </div>

        <!-- Stats -->
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

        <!-- Processing details (current file, ETA) -->
        <ProcessingDetails category="clap" {progress} />
      </div>
    {/if}
  {/if}
</div>
