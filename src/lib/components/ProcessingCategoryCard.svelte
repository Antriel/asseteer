<script lang="ts">
  import type { ProcessingCategory, CategoryProgress } from '$lib/types';
  import {
    processingState,
    getCategoryStatus,
    formatElapsed,
    isCategoryStarting,
  } from '$lib/state/tasks.svelte';
  import { settings } from '$lib/state/settings.svelte';
  import ProcessingDetails from './ProcessingDetails.svelte';

  interface Props {
    category: ProcessingCategory;
    progress: CategoryProgress | null;
    pendingCount: number;
    disabled?: boolean;
  }

  let { category, progress, pendingCount, disabled = false }: Props = $props();

  // Compute display values
  let status = $derived(getCategoryStatus(processingState, category));
  let categoryLabel = $derived(category.charAt(0).toUpperCase() + category.slice(1));
  let categoryDescription = $derived(
    category === 'image'
      ? 'Generates thumbnails and extracts dimensions'
      : 'Extracts duration, sample rate, and channel info',
  );
  let total = $derived(progress?.total || 0);
  let completed = $derived(progress?.completed || 0);
  let failed = $derived(progress?.failed || 0);
  let processed = $derived(completed + failed);
  let ratio = $derived(total === 0 ? 0 : processed / total);
  let percentage = $derived(Math.floor(ratio * 100));
  let durationMs = $derived(processingState.categoryDurationMs.get(category));
  let isStarting = $derived(isCategoryStarting(processingState, category));

  // Status badge configuration
  let statusConfig = $derived.by(() => {
    if (isStarting) {
      return {
        label: 'Starting...',
        bgClass: 'bg-blue-100 dark:bg-blue-900/30',
        textClass: 'text-blue-700 dark:text-blue-300',
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
        return {
          label: 'Completed',
          bgClass: 'bg-green-100 dark:bg-green-900/30',
          textClass: 'text-green-700 dark:text-green-300',
        };
      default:
        return {
          label: 'Idle',
          bgClass: 'bg-gray-100 dark:bg-gray-700',
          textClass: 'text-gray-700 dark:text-gray-300',
        };
    }
  });

  // Stopping wind-down state
  let isStopping = $derived(processingState.stoppingCategories.has(category));

  // Button visibility
  let canStart = $derived(status === 'idle' && !disabled && !isStarting);
  let canPause = $derived(status === 'running' && !isStopping && !isStarting);
  let canResume = $derived(status === 'paused' && !isStopping && !isStarting);
  let canStop = $derived((status === 'running' || status === 'paused') && !isStopping && !isStarting);

  // Event handlers
  async function handleStart() {
    try {
      await processingState.startProcessing(category);
    } catch (error) {
      console.error(`Failed to start ${category}:`, error);
    }
  }

  async function handlePause() {
    try {
      await processingState.pause(category);
    } catch (error) {
      console.error(`Failed to pause ${category}:`, error);
    }
  }

  async function handleResume() {
    try {
      await processingState.resume(category);
    } catch (error) {
      console.error(`Failed to resume ${category}:`, error);
    }
  }

  async function handleStop() {
    try {
      await processingState.stop(category);
    } catch (error) {
      console.error(`Failed to stop ${category}:`, error);
    }
  }
</script>

<div
  class="flex flex-col gap-3 p-4 bg-primary border border-default rounded-lg transition-opacity"
  class:opacity-50={disabled}
>
  <!-- Header: Category name and status -->
  <div class="flex items-center justify-between">
    <div class="flex items-center gap-3">
      <div>
        <h4 class="text-base font-semibold text-primary">{categoryLabel}</h4>
        <p class="text-xs text-secondary">{categoryDescription}</p>
      </div>
      <span class="text-xs px-2 py-1 rounded {statusConfig.bgClass} {statusConfig.textClass}">
        {statusConfig.label}
      </span>
      {#if status === 'completed' && durationMs}
        <span class="text-xs text-tertiary">in {formatElapsed(durationMs)}</span>
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
          class="px-3 py-1.5 text-sm font-medium text-white bg-blue-500 hover:bg-blue-600 rounded transition-colors"
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

  <!-- Progress info -->
  {#if status === 'idle' && pendingCount > 0 && !isStarting}
    <!-- Idle state: show pending count -->
    <div class="flex items-center gap-2 text-sm">
      <span class="text-secondary">Pending:</span>
      <span class="font-semibold text-primary">{pendingCount} assets</span>
    </div>
    {#if category === 'image'}
      <div class="flex items-center justify-between pt-2 border-t border-default">
        <div>
          <div class="text-sm text-primary">Pre-generate thumbnails</div>
          <div class="text-xs text-tertiary mt-0.5">
            Generate thumbnails during processing instead of on scroll. Slower processing, faster browsing.
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
    <!-- No pending items -->
    <div class="text-sm text-secondary">No assets to process</div>
  {:else if progress}
    <!-- Processing state: show progress bar and stats -->
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
            class="h-full bg-blue-500 transition-all duration-300"
            style="width: {ratio * 100}%"
          ></div>
        </div>
      </div>

      <!-- Stats: completed and failed -->
      <div class="flex items-center gap-4 text-sm">
        <div class="flex items-center gap-1.5">
          <svg class="w-4 h-4 text-green-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
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
            <svg class="w-4 h-4 text-red-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
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

      <!-- Processing details (current file, ETA, errors) -->
      <ProcessingDetails {category} {progress} />
    </div>
  {/if}
</div>
