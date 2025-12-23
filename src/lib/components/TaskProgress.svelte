<script lang="ts">
  import {
    processingState,
    getOverallProgress,
    isAnyRunning,
    isAnyPaused,
    getCategoryProgress,
  } from '$lib/state/tasks.svelte';
  import type { ProcessingCategory } from '$lib/types';
  import ProcessingCategoryCard from './ProcessingCategoryCard.svelte';
  import ClapProcessingCard from './ClapProcessingCard.svelte';

  // Note: Listeners are initialized in root layout, no need to initialize here

  // Derived state
  let overallProgress = $derived(getOverallProgress(processingState));
  let anyRunning = $derived(isAnyRunning(processingState));
  let anyPaused = $derived(isAnyPaused(processingState));

  // Categories to display
  const categories: ProcessingCategory[] = ['image', 'audio'];

  // Global control handlers
  async function handleStartAll() {
    try {
      await processingState.startAllEnabled();
    } catch (error) {
      console.error('Failed to start all:', error);
    }
  }

  async function handlePauseAll() {
    try {
      await processingState.pauseAll();
    } catch (error) {
      console.error('Failed to pause all:', error);
    }
  }

  async function handleResumeAll() {
    try {
      await processingState.resumeAll();
    } catch (error) {
      console.error('Failed to resume all:', error);
    }
  }

  async function handleStopAll() {
    try {
      await processingState.stopAll();
    } catch (error) {
      console.error('Failed to stop all:', error);
    }
  }

  // Check if any category has pending items
  let hasPendingAssets = $derived(processingState.pendingCount.total > 0);
</script>

<div class="flex flex-col gap-4 p-4 bg-secondary border border-default rounded-lg">
  <!-- Header with title and overall stats -->
  <div class="flex items-center justify-between">
    <div class="flex items-center gap-4">
      <h3 class="text-lg font-semibold text-primary">Asset Processing</h3>

      <!-- Overall stats (when processing) -->
      {#if anyRunning && overallProgress.total > 0}
        <div class="flex items-center gap-3 text-sm">
          <span class="text-secondary">Overall:</span>
          <span class="font-medium text-primary">
            {overallProgress.completed + overallProgress.failed} / {overallProgress.total}
            ({overallProgress.percentage}%)
          </span>
          {#if overallProgress.failed > 0}
            <span class="text-red-600 dark:text-red-400">
              {overallProgress.failed} failed
            </span>
          {/if}
        </div>
      {/if}
    </div>

    <!-- Global controls -->
    <div class="flex items-center gap-2">
      {#if anyRunning && !anyPaused}
        <button
          onclick={handlePauseAll}
          class="px-3 py-1.5 text-sm font-medium text-white bg-orange-500 hover:bg-orange-600 rounded transition-colors"
        >
          Pause All
        </button>
        <button
          onclick={handleStopAll}
          class="px-3 py-1.5 text-sm font-medium text-white bg-red-500 hover:bg-red-600 rounded transition-colors"
        >
          Stop All
        </button>
      {:else if anyPaused}
        <button
          onclick={handleResumeAll}
          class="px-3 py-1.5 text-sm font-medium text-white bg-green-500 hover:bg-green-600 rounded transition-colors"
        >
          Resume All
        </button>
        <button
          onclick={handleStopAll}
          class="px-3 py-1.5 text-sm font-medium text-white bg-red-500 hover:bg-red-600 rounded transition-colors"
        >
          Stop All
        </button>
      {:else if hasPendingAssets}
        <button
          onclick={handleStartAll}
          class="px-3 py-1.5 text-sm font-medium text-white bg-blue-500 hover:bg-blue-600 rounded transition-colors"
        >
          Start All
        </button>
      {/if}
    </div>
  </div>

  <!-- Category cards (vertically stacked) -->
  <div class="flex flex-col gap-3">
    {#each categories as category}
      {@const progress = getCategoryProgress(processingState, category)}
      {@const pendingCount = processingState.getPendingCountForCategory(category)}
      {@const disabled = pendingCount === 0 && !progress?.isRunning}

      <ProcessingCategoryCard
        {category}
        {progress}
        {pendingCount}
        {disabled}
      />
    {/each}

    <!-- CLAP audio embeddings processing -->
    <ClapProcessingCard />
  </div>

  <!-- Info messages -->
  {#if !hasPendingAssets && !anyRunning}
    <div class="text-center text-sm text-secondary py-4">
      No assets to process. Scan a folder to get started.
    </div>
  {/if}

  {#if anyPaused}
    <div
      class="flex items-center gap-2 p-3 bg-orange-50 dark:bg-orange-900/20 border border-orange-200 dark:border-orange-800 rounded"
    >
      <svg
        class="w-5 h-5 text-orange-500"
        fill="none"
        viewBox="0 0 24 24"
        stroke="currentColor"
      >
        <path
          stroke-linecap="round"
          stroke-linejoin="round"
          stroke-width="2"
          d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"
        />
      </svg>
      <span class="text-sm text-orange-700 dark:text-orange-300">
        Some categories are paused. Click "Resume All" or resume individual categories.
      </span>
    </div>
  {/if}
</div>
