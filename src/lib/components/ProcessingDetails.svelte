<script lang="ts">
  import type { ProcessingCategory, CategoryProgress, ProcessingErrorDetail } from '$lib/types';
  import { processingState, formatEta, formatRate } from '$lib/state/tasks.svelte';

  interface Props {
    category: ProcessingCategory;
    progress: CategoryProgress | null;
  }

  let { category, progress }: Props = $props();

  // Local state
  let errors = $state<ProcessingErrorDetail[]>([]);
  let showErrors = $state(false);
  let isRetrying = $state(false);
  let isLoadingErrors = $state(false);

  // Derived values
  let currentFile = $derived(progress?.current_file);
  let etaDisplay = $derived(formatEta(progress?.eta_seconds ?? null));
  let rateDisplay = $derived(formatRate(progress?.processing_rate ?? 0));
  let isRunning = $derived(progress?.isRunning && !progress?.isPaused);
  let hasFailures = $derived((progress?.failed ?? 0) > 0);

  async function loadErrors() {
    if (isLoadingErrors) return;
    isLoadingErrors = true;
    try {
      errors = await processingState.fetchErrors(category);
    } catch (e) {
      console.error('Failed to load errors:', e);
    } finally {
      isLoadingErrors = false;
    }
  }

  async function toggleErrors() {
    showErrors = !showErrors;
    if (showErrors && errors.length === 0) {
      await loadErrors();
    }
  }

  async function handleRetryFailed() {
    if (isRetrying) return;

    isRetrying = true;
    try {
      await processingState.retryFailed(category);
      showErrors = false;
      errors = [];
    } catch (e) {
      console.error('Retry failed:', e);
    } finally {
      isRetrying = false;
    }
  }
</script>

<div class="flex flex-col gap-3 pt-3 border-t border-default">
  <!-- Current file being processed -->
  {#if currentFile && isRunning}
    <div class="flex items-center gap-2 text-sm">
      <span class="text-secondary">Processing:</span>
      <span class="font-mono text-xs text-primary truncate max-w-xs" title={currentFile}>
        {currentFile}
      </span>
    </div>
  {/if}

  <!-- Processing stats row -->
  {#if isRunning}
    <div class="flex items-center gap-4 text-sm">
      <!-- Rate -->
      <div class="flex items-center gap-1.5">
        <span class="text-secondary">Rate:</span>
        <span class="font-medium text-primary">{rateDisplay}</span>
      </div>

      <!-- ETA -->
      <div class="flex items-center gap-1.5">
        <span class="text-secondary">ETA:</span>
        <span class="font-medium text-primary">{etaDisplay}</span>
      </div>
    </div>
  {/if}

  <!-- Errors section -->
  {#if hasFailures}
    <div class="flex flex-col gap-2">
      <div class="flex items-center justify-between">
        <button
          onclick={toggleErrors}
          class="flex items-center gap-2 text-sm text-red-600 dark:text-red-400 hover:underline"
        >
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
            />
          </svg>
          {progress?.failed} error{(progress?.failed ?? 0) > 1 ? 's' : ''}
          <svg
            class="w-3 h-3 transition-transform {showErrors ? 'rotate-180' : ''}"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
          >
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M19 9l-7 7-7-7"
            />
          </svg>
        </button>

        <!-- Retry button -->
        {#if !progress?.isRunning}
          <button
            onclick={handleRetryFailed}
            disabled={isRetrying}
            class="px-3 py-1 text-xs font-medium text-white bg-orange-500 hover:bg-orange-600
                   disabled:opacity-50 disabled:cursor-not-allowed rounded transition-colors"
          >
            {isRetrying ? 'Retrying...' : 'Retry Failed'}
          </button>
        {/if}
      </div>

      <!-- Error list (collapsible) -->
      {#if showErrors}
        <div class="max-h-48 overflow-y-auto bg-red-50 dark:bg-red-900/20 rounded p-2">
          {#if isLoadingErrors}
            <div class="text-xs text-secondary text-center py-2">Loading errors...</div>
          {:else if errors.length === 0}
            <div class="text-xs text-secondary text-center py-2">No error details available</div>
          {:else}
            <ul class="space-y-2">
              {#each errors as error}
                <li class="text-xs">
                  <div class="font-medium text-primary truncate" title={error.path}>
                    {error.filename}
                  </div>
                  <div class="text-red-600 dark:text-red-400 truncate" title={error.error_message}>
                    {error.error_message}
                  </div>
                  {#if error.retry_count > 0}
                    <div class="text-secondary">
                      Retried {error.retry_count} time{error.retry_count > 1 ? 's' : ''}
                    </div>
                  {/if}
                </li>
              {/each}
            </ul>
          {/if}
        </div>
      {/if}
    </div>
  {/if}
</div>
