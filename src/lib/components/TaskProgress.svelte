<script lang="ts">
  import { processingState, getStatusColor } from '$lib/state/tasks.svelte';
  import { onMount, onDestroy } from 'svelte';

  // Initialize listeners on mount
  onMount(async () => {
    await processingState.initializeListeners();
    await processingState.refreshProgress();
    await processingState.refreshPendingCount();
  });

  // Cleanup on destroy
  onDestroy(() => {
    processingState.cleanup();
  });

  // Handlers
  async function handleStart() {
    try {
      await processingState.startProcessing();
    } catch (error) {
      console.error('Failed to start processing:', error);
    }
  }

  async function handlePause() {
    try {
      await processingState.pause();
    } catch (error) {
      console.error('Failed to pause:', error);
    }
  }

  async function handleResume() {
    try {
      await processingState.resume();
    } catch (error) {
      console.error('Failed to resume:', error);
    }
  }

  async function handleStop() {
    try {
      await processingState.stop();
    } catch (error) {
      console.error('Failed to stop:', error);
    }
  }
</script>

<div class="flex flex-col gap-4 p-4 bg-secondary border border-default rounded-lg">
  <!-- Header with controls -->
  <div class="flex items-center justify-between">
    <h3 class="text-lg font-semibold text-primary">Asset Processing</h3>

    <div class="flex items-center gap-2">
      {#if processingState.isRunning && !processingState.isPaused}
        <button
          onclick={handlePause}
          class="px-3 py-1.5 text-sm font-medium text-white bg-orange-500 hover:bg-orange-600 rounded transition-colors"
        >
          Pause
        </button>
        <button
          onclick={handleStop}
          class="px-3 py-1.5 text-sm font-medium text-white bg-red-500 hover:bg-red-600 rounded transition-colors"
        >
          Stop
        </button>
      {:else if processingState.isPaused}
        <button
          onclick={handleResume}
          class="px-3 py-1.5 text-sm font-medium text-white bg-green-500 hover:bg-green-600 rounded transition-colors"
        >
          Resume
        </button>
        <button
          onclick={handleStop}
          class="px-3 py-1.5 text-sm font-medium text-white bg-red-500 hover:bg-red-600 rounded transition-colors"
        >
          Stop
        </button>
      {:else if processingState.pendingCount.total > 0}
        <button
          onclick={handleStart}
          class="px-3 py-1.5 text-sm font-medium text-white bg-blue-500 hover:bg-blue-600 rounded transition-colors"
        >
          Start Processing
        </button>
      {/if}
    </div>
  </div>

  <!-- Statistics Grid (when processing is running) -->
  {#if processingState.isRunning && processingState.total > 0}
    <div class="grid grid-cols-4 gap-3">
      <div class="flex flex-col items-center p-3 bg-primary border border-default rounded">
        <span class="text-2xl font-bold text-primary">{processingState.total}</span>
        <span class="text-xs text-secondary">Total</span>
      </div>

      <div class="flex flex-col items-center p-3 bg-primary border border-default rounded">
        <span class="text-2xl font-bold text-green-500">{processingState.completed}</span>
        <span class="text-xs text-secondary">Completed</span>
      </div>

      <div class="flex flex-col items-center p-3 bg-primary border border-default rounded">
        <span class="text-2xl font-bold text-red-500">{processingState.failed}</span>
        <span class="text-xs text-secondary">Failed</span>
      </div>

      <div class="flex flex-col items-center p-3 bg-primary border border-default rounded">
        <span class="text-2xl font-bold {getStatusColor(processingState)}">{processingState.getStatusText()}</span>
        <span class="text-xs text-secondary">Status</span>
      </div>
    </div>

    <!-- Progress Bar -->
    <div class="flex flex-col gap-2">
      <div class="flex items-center justify-between text-sm">
        <span class="text-secondary">Progress</span>
        <span class="font-medium text-primary">
          {processingState.completed + processingState.failed} / {processingState.total}
          ({processingState.getProgressPercentage()}%)
        </span>
      </div>

      <div class="w-full h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
        <div
          class="h-full bg-blue-500 transition-all duration-300"
          style="width: {processingState.getProgressPercentage()}%"
        ></div>
      </div>
    </div>
  {:else if processingState.pendingCount.total > 0}
    <!-- Pending assets info (when not processing) -->
    <div class="flex flex-col gap-3 py-4">
      <div class="text-center">
        <div class="text-3xl font-bold text-primary mb-2">
          {processingState.pendingCount.total}
        </div>
        <div class="text-sm text-secondary">
          Assets ready to process
        </div>
      </div>

      <div class="flex items-center justify-center gap-4 text-sm">
        <div class="flex items-center gap-2">
          <span class="text-secondary">Images:</span>
          <span class="font-semibold text-primary">{processingState.pendingCount.images}</span>
        </div>
        <span class="text-gray-300 dark:text-gray-600">•</span>
        <div class="flex items-center gap-2">
          <span class="text-secondary">Audio:</span>
          <span class="font-semibold text-primary">{processingState.pendingCount.audio}</span>
        </div>
      </div>
    </div>
  {:else}
    <div class="text-center text-sm text-secondary py-8">
      No assets to process. Scan a folder to get started.
    </div>
  {/if}

  <!-- Paused state warning -->
  {#if processingState.isPaused}
    <div class="flex items-center gap-2 p-3 bg-orange-50 dark:bg-orange-900/20 border border-orange-200 dark:border-orange-800 rounded">
      <svg class="w-5 h-5 text-orange-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
      </svg>
      <span class="text-sm text-orange-700 dark:text-orange-300">
        Processing is paused. Click "Resume" to continue.
      </span>
    </div>
  {/if}
</div>
