<script lang="ts">
  import {
    processingState,
    getOverallProgress,
    isAnyRunning,
    isAnyPaused,
    getCategoryProgress,
  } from '$lib/state/tasks.svelte';
  import type { ProcessingCategory } from '$lib/types';
  import ProcessingCategoryCard from '$lib/components/ProcessingCategoryCard.svelte';
  import ClapProcessingCard from '$lib/components/ClapProcessingCard.svelte';
  import { onMount } from 'svelte';


  // Refresh state on mount (listeners already initialized in root layout)
  onMount(async () => {
    await processingState.refreshProgress();
    await processingState.refreshPendingCount();
  });

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

<div class="flex flex-col h-full overflow-auto p-6">
  <div class="w-full max-w-3xl mx-auto">
  <!-- Page Header -->
  <div class="mb-6">
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-bold text-primary">Processing</h1>
        <p class="text-sm text-secondary mt-1">
          Extract metadata, generate thumbnails, and enable search for your assets
        </p>
      </div>

      <!-- Global controls -->
      <div class="flex items-center gap-2">
        {#if anyRunning && !anyPaused}
          <button
            onclick={handlePauseAll}
            class="px-4 py-2 text-sm font-medium text-white bg-warning hover:opacity-90 rounded-lg transition-default"
          >
            Pause All
          </button>
          <button
            onclick={handleStopAll}
            class="px-4 py-2 text-sm font-medium text-white bg-error hover:opacity-90 rounded-lg transition-default"
          >
            Stop All
          </button>
        {:else if anyPaused}
          <button
            onclick={handleResumeAll}
            class="px-4 py-2 text-sm font-medium text-white bg-success hover:opacity-90 rounded-lg transition-default"
          >
            Resume All
          </button>
          <button
            onclick={handleStopAll}
            class="px-4 py-2 text-sm font-medium text-white bg-error hover:opacity-90 rounded-lg transition-default"
          >
            Stop All
          </button>
        {:else if hasPendingAssets}
          <button
            onclick={handleStartAll}
            class="px-4 py-2 text-sm font-medium text-white bg-accent hover:opacity-90 rounded-lg transition-default"
          >
            Start All
          </button>
        {/if}
      </div>
    </div>

    <!-- Overall progress bar (always reserves space to avoid layout shift) -->
    <div class="mt-4 transition-opacity duration-200" class:opacity-0={!anyRunning || overallProgress.total === 0}>
      <div class="flex items-center justify-between mb-1.5 text-xs">
        <span class="text-secondary font-medium">Overall progress</span>
        <span class="text-primary font-medium">
          {overallProgress.completed + overallProgress.failed} / {overallProgress.total} &middot; {overallProgress.percentage}%{#if overallProgress.failed > 0} &middot; <span class="text-error">{overallProgress.failed} failed</span>{/if}
        </span>
      </div>
      <div class="h-1.5 bg-tertiary rounded-full overflow-hidden">
        <div
          class="h-full bg-accent transition-all duration-300"
          style="width: {overallProgress.percentage}%"
        ></div>
      </div>
    </div>
  </div>

  <!-- Category cards -->
  <div class="flex flex-col gap-4">
    {#each categories as category}
      {@const progress = getCategoryProgress(processingState, category)}
      {@const pendingCount = processingState.getPendingCountForCategory(category)}
      {@const disabled = pendingCount === 0 && !progress?.isRunning}

      <ProcessingCategoryCard {category} {progress} {pendingCount} {disabled} />
    {/each}

    <!-- CLAP audio embeddings processing -->
    <ClapProcessingCard />
  </div>

  <!-- Info messages -->
  {#if !hasPendingAssets && !anyRunning}
    <div class="flex flex-col items-center justify-center py-12 text-center">
      <svg
        class="w-16 h-16 text-tertiary mb-4"
        fill="none"
        stroke="currentColor"
        viewBox="0 0 24 24"
      >
        <path
          stroke-linecap="round"
          stroke-linejoin="round"
          stroke-width="1.5"
          d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"
        />
      </svg>
      <p class="text-primary font-medium">All assets processed</p>
      <p class="text-sm text-secondary mt-1">Scan a folder to discover new assets</p>
      <a
        href="/sources"
        class="mt-4 px-4 py-2 text-sm font-medium text-white bg-accent hover:opacity-90 rounded-lg transition-default"
      >
        Add Folder
      </a>
    </div>
  {/if}

  {#if anyPaused}
    <div class="flex items-center gap-3 p-4 bg-warning/10 border border-warning/30 rounded-lg mt-4">
      <svg
        class="w-5 h-5 text-warning flex-shrink-0"
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
      <span class="text-sm text-warning">
        Some categories are paused. Click "Resume All" or resume individual categories.
      </span>
    </div>
  {/if}
  </div>
</div>
