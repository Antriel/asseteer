<script lang="ts">
  import { tasksState, getTaskStatusColor, formatTaskType } from '$lib/state/tasks.svelte';
  import { onMount, onDestroy } from 'svelte';

  // Initialize listeners on mount
  onMount(async () => {
    await tasksState.initializeListeners();
    await tasksState.refreshStats();
  });

  // Cleanup on destroy
  onDestroy(() => {
    tasksState.cleanup();
  });

  // Derived values
  const stats = $derived(tasksState.stats);
  const isProcessing = $derived(tasksState.isProcessing);
  const currentProgress = $derived(tasksState.currentProgress);
  const activeTasks = $derived(tasksState.getActiveTasks());

  // Handlers
  async function handlePauseAll() {
    try {
      await tasksState.pauseAll();
    } catch (error) {
      console.error('Failed to pause all:', error);
    }
  }

  async function handleResumeAll() {
    try {
      await tasksState.resumeAll();
    } catch (error) {
      console.error('Failed to resume all:', error);
    }
  }

  async function handleStartProcessing() {
    try {
      await tasksState.startProcessing();
    } catch (error) {
      console.error('Failed to start processing:', error);
    }
  }

  // Calculate overall progress percentage
  const overallProgress = $derived(() => {
    if (stats.total === 0) return 0;
    return Math.round((stats.complete / stats.total) * 100);
  });
</script>

<div class="flex flex-col gap-4 p-4 bg-secondary border border-default rounded-lg">
  <!-- Header with controls -->
  <div class="flex items-center justify-between">
    <h3 class="text-lg font-semibold text-primary">Task Processing</h3>

    <div class="flex items-center gap-2">
      {#if isProcessing}
        <button
          onclick={handlePauseAll}
          class="px-3 py-1.5 text-sm font-medium text-white bg-orange-500 hover:bg-orange-600 rounded transition-colors"
        >
          Pause All
        </button>
      {:else if stats.paused > 0}
        <button
          onclick={handleResumeAll}
          class="px-3 py-1.5 text-sm font-medium text-white bg-green-500 hover:bg-green-600 rounded transition-colors"
        >
          Resume All
        </button>
      {:else if stats.pending > 0}
        <button
          onclick={handleStartProcessing}
          class="px-3 py-1.5 text-sm font-medium text-white bg-blue-500 hover:bg-blue-600 rounded transition-colors"
        >
          Start Processing
        </button>
      {/if}
    </div>
  </div>

  <!-- Statistics -->
  <div class="grid grid-cols-4 gap-3">
    <div class="flex flex-col items-center p-3 bg-primary border border-default rounded">
      <span class="text-2xl font-bold text-primary">{stats.pending}</span>
      <span class="text-xs text-secondary">Pending</span>
    </div>

    <div class="flex flex-col items-center p-3 bg-primary border border-default rounded">
      <span class="text-2xl font-bold text-yellow-500">{stats.processing + stats.queued}</span>
      <span class="text-xs text-secondary">Active</span>
    </div>

    <div class="flex flex-col items-center p-3 bg-primary border border-default rounded">
      <span class="text-2xl font-bold text-green-500">{stats.complete}</span>
      <span class="text-xs text-secondary">Complete</span>
    </div>

    <div class="flex flex-col items-center p-3 bg-primary border border-default rounded">
      <span class="text-2xl font-bold text-red-500">{stats.error}</span>
      <span class="text-xs text-secondary">Error</span>
    </div>
  </div>

  <!-- Overall progress bar -->
  {#if stats.total > 0}
    <div class="flex flex-col gap-2">
      <div class="flex items-center justify-between text-sm">
        <span class="text-secondary">Overall Progress</span>
        <span class="font-medium text-primary">{stats.complete} / {stats.total} ({overallProgress()}%)</span>
      </div>

      <div class="w-full h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
        <div
          class="h-full bg-blue-500 transition-all duration-300"
          style="width: {overallProgress()}%"
        ></div>
      </div>
    </div>
  {/if}

  <!-- Current task progress -->
  {#if currentProgress}
    <div class="flex flex-col gap-2 p-3 bg-primary border border-default rounded">
      <div class="flex items-center justify-between">
        <span class="text-sm font-medium text-primary">
          {formatTaskType(currentProgress.task_type)}
        </span>
        <span class="text-xs {getTaskStatusColor(currentProgress.status)}">
          {currentProgress.status}
        </span>
      </div>

      {#if currentProgress.current_file}
        <span class="text-xs text-secondary truncate">{currentProgress.current_file}</span>
      {/if}

      <div class="flex items-center gap-2">
        <div class="flex-1 h-1.5 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
          <div
            class="h-full bg-green-500 transition-all duration-300"
            style="width: {(currentProgress.progress_current / currentProgress.progress_total) * 100}%"
          ></div>
        </div>
        <span class="text-xs font-medium text-secondary">
          {currentProgress.progress_current}/{currentProgress.progress_total}
        </span>
      </div>
    </div>
  {/if}

  <!-- Active tasks list -->
  {#if activeTasks.length > 0}
    <div class="flex flex-col gap-2">
      <h4 class="text-sm font-medium text-secondary">Active Tasks ({activeTasks.length})</h4>
      <div class="flex flex-col gap-1 max-h-32 overflow-y-auto">
        {#each activeTasks as task}
          <div class="flex items-center justify-between p-2 bg-primary border border-default rounded text-xs">
            <div class="flex items-center gap-2">
              <span class="font-medium text-primary">{formatTaskType(task.task_type)}</span>
              <span class={getTaskStatusColor(task.status)}>• {task.status}</span>
            </div>

            <div class="flex items-center gap-2">
              <span class="text-secondary">
                {task.progress_current}/{task.progress_total}
              </span>

              {#if task.status === 'processing'}
                <button
                  onclick={() => tasksState.pauseTask(task.id)}
                  class="px-2 py-0.5 text-xs text-white bg-orange-500 hover:bg-orange-600 rounded"
                >
                  Pause
                </button>
              {:else if task.status === 'paused'}
                <button
                  onclick={() => tasksState.resumeTask(task.id)}
                  class="px-2 py-0.5 text-xs text-white bg-green-500 hover:bg-green-600 rounded"
                >
                  Resume
                </button>
              {/if}

              <button
                onclick={() => tasksState.cancelTask(task.id)}
                class="px-2 py-0.5 text-xs text-white bg-red-500 hover:bg-red-600 rounded"
              >
                Cancel
              </button>
            </div>
          </div>
        {/each}
      </div>
    </div>
  {/if}

  <!-- Paused tasks info -->
  {#if stats.paused > 0 && !isProcessing}
    <div class="flex items-center gap-2 p-3 bg-orange-50 dark:bg-orange-900/20 border border-orange-200 dark:border-orange-800 rounded">
      <svg class="w-5 h-5 text-orange-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
      </svg>
      <span class="text-sm text-orange-700 dark:text-orange-300">
        {stats.paused} task{stats.paused === 1 ? '' : 's'} paused. Click "Resume All" to continue.
      </span>
    </div>
  {/if}
</div>
