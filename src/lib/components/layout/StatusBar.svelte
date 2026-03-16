<script lang="ts">
  import {
    processingState,
    isAnyRunning,
    isAnyPaused,
    getOverallProgress,
    getCategoryProgress,
    getCategoryStatus,
  } from '$lib/state/tasks.svelte';
  import { uiState } from '$lib/state/ui.svelte';
  import type { ProcessingCategory } from '$lib/types';

  const isProcessing = $derived(isAnyRunning(processingState));
  const isPaused = $derived(isAnyPaused(processingState));
  const overall = $derived(getOverallProgress(processingState));
  const pendingTotal = $derived(processingState.pendingCount.total);

  const categories: ProcessingCategory[] = ['image', 'audio', 'clap'];

  function getCategoryLabel(category: ProcessingCategory): string {
    if (category === 'image') return 'IMG';
    if (category === 'audio') return 'AUD';
    if (category === 'clap') return 'CLAP';
    return category;
  }

  function getCategoryProgressPercent(category: ProcessingCategory): number {
    const progress = getCategoryProgress(processingState, category);
    if (!progress || progress.total === 0) return 0;
    return Math.round(((progress.completed + progress.failed) / progress.total) * 100);
  }
</script>

<footer class="h-10 flex items-center px-4 bg-secondary border-t border-default text-sm">
  <!-- Scan Status -->
  {#if uiState.isScanning}
    <a href="/scan" class="flex items-center gap-3 hover:text-accent transition-colors mr-4">
      <div class="flex items-center gap-2">
        <div class="w-2 h-2 rounded-full bg-accent animate-pulse"></div>
        <span class="text-primary font-medium">Scanning</span>
        {#if uiState.scanDetails.phase === 'discovering'}
          <span class="text-secondary">{uiState.scanDetails.filesFound} found</span>
        {:else if uiState.scanDetails.phase === 'inserting'}
          {@const pct = uiState.scanDetails.filesTotal > 0 ? Math.round((uiState.scanDetails.filesInserted / uiState.scanDetails.filesTotal) * 100) : 0}
          <span class="text-secondary">{pct}%</span>
        {/if}
      </div>
    </a>
    <div class="w-px h-5 bg-tertiary mr-4"></div>
  {/if}

  <!-- Processing Status -->
  <a href="/processing" class="flex items-center gap-3 hover:text-accent transition-colors">
    <!-- Status indicator -->
    {#if isProcessing && !isPaused}
      <div class="flex items-center gap-2">
        <div class="w-2 h-2 rounded-full bg-accent animate-pulse"></div>
        <span class="text-primary font-medium">Processing</span>
        <span class="text-secondary">{overall.completed}/{overall.total}</span>
      </div>
    {:else if isPaused}
      <div class="flex items-center gap-2">
        <div class="w-2 h-2 rounded-full bg-warning"></div>
        <span class="text-warning font-medium">Paused</span>
        <span class="text-secondary">{overall.completed}/{overall.total}</span>
      </div>
    {:else if processingState.lastRunResult}
      {@const result = processingState.lastRunResult}
      <div class="flex items-center gap-2">
        {#if result.failed > 0}
          <div class="w-2 h-2 rounded-full bg-error"></div>
          <span class="text-primary font-medium">Complete</span>
          <span class="text-secondary">{result.completed} processed</span>
          <span class="text-error">{result.failed} failed</span>
        {:else}
          <div class="w-2 h-2 rounded-full bg-success"></div>
          <span class="text-primary font-medium">Complete</span>
          <span class="text-secondary">{result.completed} processed</span>
        {/if}
      </div>
    {:else}
      <div class="flex items-center gap-2">
        <div class="w-2 h-2 rounded-full bg-tertiary"></div>
        <span class="text-secondary">Idle</span>
        {#if pendingTotal > 0}
          <span class="text-tertiary">{pendingTotal} pending</span>
        {:else}
          <span class="text-tertiary">All processed</span>
        {/if}
      </div>
    {/if}
  </a>

  <!-- Spacer -->
  <div class="flex-1"></div>

  <!-- Category mini-indicators (only show when processing or has progress) -->
  {#if isProcessing || overall.total > 0}
    <div class="flex items-center gap-4">
      {#each categories as category}
        {@const status = getCategoryStatus(processingState, category)}
        {@const percent = getCategoryProgressPercent(category)}
        {@const progress = getCategoryProgress(processingState, category)}
        {#if progress && progress.total > 0}
          <div class="flex items-center gap-1.5 text-xs">
            <span class="text-tertiary">{getCategoryLabel(category)}</span>
            <div class="w-12 h-1.5 rounded-full bg-tertiary overflow-hidden">
              <div
                class="h-full rounded-full transition-all duration-300
                       {status === 'running' ? 'bg-accent' : status === 'paused' ? 'bg-warning' : status === 'completed' ? 'bg-success' : 'bg-secondary'}"
                style="width: {percent}%"
              ></div>
            </div>
            <span class="text-tertiary w-8">{percent}%</span>
          </div>
        {/if}
      {/each}
    </div>
  {/if}

  <!-- View details link -->
  <a href="/processing" class="ml-4 text-xs text-tertiary hover:text-accent transition-colors">
    Details
  </a>
</footer>
