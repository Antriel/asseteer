<script lang="ts">
  import {
    processingState,
    getStatusColor,
    getOverallProgress,
    isAnyRunning,
    isAnyPaused,
    getCategoryProgress,
  } from '$lib/state/tasks.svelte';
  import type { ProcessingCategory } from '$lib/types';
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

  // Derived state
  let overallProgress = $derived(getOverallProgress(processingState));
  let anyRunning = $derived(isAnyRunning(processingState));
  let anyPaused = $derived(isAnyPaused(processingState));
  let hasEnabledCategories = $derived(processingState.enabledCategories.size > 0);
  let hasPendingAssets = $derived(processingState.pendingCount.total > 0);

  // Handlers
  async function handleStart() {
    try {
      await processingState.startAllEnabled();
    } catch (error) {
      console.error('Failed to start processing:', error);
    }
  }

  async function handlePauseAll() {
    try {
      await processingState.pauseAll();
    } catch (error) {
      console.error('Failed to pause:', error);
    }
  }

  async function handleResumeAll() {
    try {
      await processingState.resumeAll();
    } catch (error) {
      console.error('Failed to resume:', error);
    }
  }

  async function handleStopAll() {
    try {
      await processingState.stopAll();
    } catch (error) {
      console.error('Failed to stop:', error);
    }
  }

  function handleToggleCategory(category: ProcessingCategory) {
    processingState.toggleCategory(category);
  }

  // Category display helpers
  function getCategoryLabel(category: ProcessingCategory): string {
    return category.charAt(0).toUpperCase() + category.slice(1);
  }

  function getCategoryPendingCount(category: ProcessingCategory): number {
    if (category === 'image') return processingState.pendingCount.images;
    if (category === 'audio') return processingState.pendingCount.audio;
    return 0;
  }

  function getCategoryProgressPercentage(category: ProcessingCategory): number {
    const progress = getCategoryProgress(processingState, category);
    if (!progress || progress.total === 0) return 0;
    return Math.round(((progress.completed + progress.failed) / progress.total) * 100);
  }
</script>

<div class="flex flex-col gap-4 p-4 bg-secondary border border-default rounded-lg">
  <!-- Header with controls -->
  <div class="flex items-center justify-between">
    <h3 class="text-lg font-semibold text-primary">Asset Processing</h3>

    <div class="flex items-center gap-2">
      {#if anyRunning && !anyPaused}
        <button
          onclick={handlePauseAll}
          class="btn-action bg-orange-500 hover:bg-orange-600"
        >
          Pause All
        </button>
        <button
          onclick={handleStopAll}
          class="btn-action bg-red-500 hover:bg-red-600"
        >
          Stop All
        </button>
      {:else if anyPaused}
        <button
          onclick={handleResumeAll}
          class="btn-action bg-green-500 hover:bg-green-600"
        >
          Resume All
        </button>
        <button
          onclick={handleStopAll}
          class="btn-action bg-red-500 hover:bg-red-600"
        >
          Stop All
        </button>
      {:else if hasPendingAssets && hasEnabledCategories}
        <button
          onclick={handleStart}
          class="btn-action bg-blue-500 hover:bg-blue-600"
        >
          Start Processing
        </button>
      {/if}
    </div>
  </div>

  <!-- Category toggles -->
  {#if !anyRunning}
    <div class="flex flex-col gap-2">
      <div class="text-sm font-medium text-secondary">Processing Categories:</div>
      <div class="flex items-center gap-4">
        <label class="flex items-center gap-2 cursor-pointer">
          <input
            type="checkbox"
            checked={processingState.enabledCategories.has('image')}
            onchange={() => handleToggleCategory('image')}
            class="w-4 h-4 text-blue-600 bg-gray-100 border-gray-300 rounded focus:ring-blue-500 dark:focus:ring-blue-600 dark:ring-offset-gray-800 focus:ring-2 dark:bg-gray-700 dark:border-gray-600"
          />
          <span class="text-sm text-primary">
            Images ({processingState.pendingCount.images} pending)
          </span>
        </label>

        <label class="flex items-center gap-2 cursor-pointer">
          <input
            type="checkbox"
            checked={processingState.enabledCategories.has('audio')}
            onchange={() => handleToggleCategory('audio')}
            class="w-4 h-4 text-blue-600 bg-gray-100 border-gray-300 rounded focus:ring-blue-500 dark:focus:ring-blue-600 dark:ring-offset-gray-800 focus:ring-2 dark:bg-gray-700 dark:border-gray-600"
          />
          <span class="text-sm text-primary">
            Audio ({processingState.pendingCount.audio} pending)
          </span>
        </label>
      </div>
    </div>
  {/if}

  <!-- Overall Statistics (when processing is running) -->
  {#if anyRunning && overallProgress.total > 0}
    <div class="grid grid-cols-4 gap-3">
      <div class="flex flex-col items-center p-3 bg-primary border border-default rounded">
        <span class="text-2xl font-bold text-primary">{overallProgress.total}</span>
        <span class="text-xs text-secondary">Total</span>
      </div>

      <div class="flex flex-col items-center p-3 bg-primary border border-default rounded">
        <span class="text-2xl font-bold text-green-500">{overallProgress.completed}</span>
        <span class="text-xs text-secondary">Completed</span>
      </div>

      <div class="flex flex-col items-center p-3 bg-primary border border-default rounded">
        <span class="text-2xl font-bold text-red-500">{overallProgress.failed}</span>
        <span class="text-xs text-secondary">Failed</span>
      </div>

      <div class="flex flex-col items-center p-3 bg-primary border border-default rounded">
        <span class="text-2xl font-bold {getStatusColor(processingState)}">
          {anyPaused ? 'Paused' : 'Running'}
        </span>
        <span class="text-xs text-secondary">Status</span>
      </div>
    </div>

    <!-- Overall Progress Bar -->
    <div class="flex flex-col gap-2">
      <div class="flex items-center justify-between text-sm">
        <span class="text-secondary">Overall Progress</span>
        <span class="font-medium text-primary">
          {overallProgress.completed + overallProgress.failed} / {overallProgress.total}
          ({overallProgress.percentage}%)
        </span>
      </div>

      <div class="w-full h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
        <div
          class="h-full bg-blue-500 transition-all duration-300"
          style="width: {overallProgress.percentage}%"
        ></div>
      </div>
    </div>

    <!-- Per-category breakdown -->
    <div class="flex flex-col gap-2">
      <div class="text-sm font-medium text-secondary">Category Progress:</div>
      {#each Array.from(processingState.categoryProgress.entries()) as [category, progress]}
        <div class="flex items-center justify-between text-sm p-2 bg-primary border border-default rounded">
          <div class="flex items-center gap-2">
            <span class="font-medium text-primary">{getCategoryLabel(category)}:</span>
            <span class="text-secondary">
              {progress.completed + progress.failed} / {progress.total}
              ({getCategoryProgressPercentage(category)}%)
            </span>
          </div>
          <div class="flex items-center gap-2">
            {#if progress.isRunning && progress.isPaused}
              <span class="text-xs px-2 py-1 bg-orange-100 dark:bg-orange-900/30 text-orange-700 dark:text-orange-300 rounded">
                Paused
              </span>
            {:else if progress.isRunning}
              <span class="text-xs px-2 py-1 bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300 rounded">
                Running
              </span>
            {:else}
              <span class="text-xs px-2 py-1 bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded">
                Idle
              </span>
            {/if}
            <span class="text-xs text-green-600 dark:text-green-400">
              ✓ {progress.completed}
            </span>
            {#if progress.failed > 0}
              <span class="text-xs text-red-600 dark:text-red-400">
                ✗ {progress.failed}
              </span>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {:else if hasPendingAssets}
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
  {#if anyPaused}
    <div class="flex items-center gap-2 p-3 bg-orange-50 dark:bg-orange-900/20 border border-orange-200 dark:border-orange-800 rounded">
      <svg class="w-5 h-5 text-orange-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
      </svg>
      <span class="text-sm text-orange-700 dark:text-orange-300">
        Some categories are paused. Click "Resume All" to continue.
      </span>
    </div>
  {/if}

  <!-- No categories selected warning -->
  {#if !anyRunning && !hasEnabledCategories && hasPendingAssets}
    <div class="flex items-center gap-2 p-3 bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800 rounded">
      <svg class="w-5 h-5 text-yellow-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
      </svg>
      <span class="text-sm text-yellow-700 dark:text-yellow-300">
        Select at least one category to start processing.
      </span>
    </div>
  {/if}
</div>
