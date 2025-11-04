import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { ProcessingTask, TaskProgress, TaskStats } from '$lib/types';

/**
 * Tasks state management - handles task processing, progress tracking, and control
 */
class TasksState {
  // Active tasks being processed
  tasks = $state<ProcessingTask[]>([]);

  // Statistics
  stats = $state<TaskStats>({
    total: 0,
    pending: 0,
    queued: 0,
    processing: 0,
    paused: 0,
    complete: 0,
    error: 0,
    cancelled: 0,
  });

  // Processing state
  isProcessing = $state(false);

  // Progress tracking
  currentProgress = $state<TaskProgress | null>(null);

  // Event listeners
  private unlistenFns: UnlistenFn[] = [];

  /**
   * Initialize event listeners for task events
   */
  async initializeListeners() {
    // Clean up existing listeners
    this.cleanup();

    // Listen for task-started events
    const unlistenStarted = await listen<TaskProgress>('task-started', (event) => {
      console.log('[Tasks] Task started:', event.payload);
      this.isProcessing = true;
      this.currentProgress = event.payload;
      this.refreshTasks();
    });

    // Listen for task-progress events
    const unlistenProgress = await listen<TaskProgress>('task-progress', (event) => {
      console.log('[Tasks] Task progress:', event.payload);
      this.currentProgress = event.payload;
      this.refreshTasks();
    });

    // Listen for task-completed events
    const unlistenCompleted = await listen<TaskProgress>('task-completed', (event) => {
      console.log('[Tasks] Task completed:', event.payload);
      this.currentProgress = null;
      this.refreshTasks();
      this.refreshStats();
    });

    this.unlistenFns = [unlistenStarted, unlistenProgress, unlistenCompleted];
  }

  /**
   * Start processing tasks
   */
  async startProcessing(taskType?: string, assetType?: string) {
    try {
      const taskIds = await invoke<number[]>('start_processing', {
        taskType,
        assetType,
      });
      console.log('[Tasks] Started processing:', taskIds.length, 'tasks');
      this.isProcessing = true;
      this.refreshTasks();
      this.refreshStats();
      return taskIds;
    } catch (error) {
      console.error('[Tasks] Failed to start processing:', error);
      throw error;
    }
  }

  /**
   * Pause a specific task
   */
  async pauseTask(taskId: number) {
    try {
      await invoke('pause_task', { taskId });
      console.log('[Tasks] Paused task:', taskId);
      this.refreshTasks();
    } catch (error) {
      console.error('[Tasks] Failed to pause task:', error);
      throw error;
    }
  }

  /**
   * Resume a specific task
   */
  async resumeTask(taskId: number) {
    try {
      await invoke('resume_task', { taskId });
      console.log('[Tasks] Resumed task:', taskId);
      this.refreshTasks();
    } catch (error) {
      console.error('[Tasks] Failed to resume task:', error);
      throw error;
    }
  }

  /**
   * Cancel a specific task
   */
  async cancelTask(taskId: number) {
    try {
      await invoke('cancel_task', { taskId });
      console.log('[Tasks] Cancelled task:', taskId);
      this.refreshTasks();
    } catch (error) {
      console.error('[Tasks] Failed to cancel task:', error);
      throw error;
    }
  }

  /**
   * Pause all active tasks
   */
  async pauseAll() {
    try {
      await invoke('pause_all_tasks');
      console.log('[Tasks] Paused all tasks');
      this.refreshTasks();
    } catch (error) {
      console.error('[Tasks] Failed to pause all tasks:', error);
      throw error;
    }
  }

  /**
   * Resume all paused tasks
   */
  async resumeAll() {
    try {
      await invoke('resume_all_tasks');
      console.log('[Tasks] Resumed all tasks');
      this.refreshTasks();
    } catch (error) {
      console.error('[Tasks] Failed to resume all tasks:', error);
      throw error;
    }
  }

  /**
   * Load all tasks (optionally filtered by status)
   */
  async loadTasks(status?: string) {
    try {
      const tasks = await invoke<ProcessingTask[]>('get_tasks', { status });
      this.tasks = tasks;
      return tasks;
    } catch (error) {
      console.error('[Tasks] Failed to load tasks:', error);
      throw error;
    }
  }

  /**
   * Refresh tasks (reload from backend)
   */
  async refreshTasks() {
    await this.loadTasks();
  }

  /**
   * Load task statistics
   */
  async loadStats() {
    try {
      const stats = await invoke<TaskStats>('get_task_stats');
      this.stats = stats;

      // Update isProcessing based on stats
      this.isProcessing = stats.processing > 0 || stats.queued > 0;

      return stats;
    } catch (error) {
      console.error('[Tasks] Failed to load stats:', error);
      throw error;
    }
  }

  /**
   * Refresh statistics
   */
  async refreshStats() {
    await this.loadStats();
  }

  /**
   * Get tasks by status
   */
  getTasksByStatus(status: string): ProcessingTask[] {
    return this.tasks.filter((t) => t.status === status);
  }

  /**
   * Get active tasks (processing or queued)
   */
  getActiveTasks(): ProcessingTask[] {
    return this.tasks.filter((t) => t.status === 'processing' || t.status === 'queued');
  }

  /**
   * Clean up event listeners
   */
  cleanup() {
    this.unlistenFns.forEach((fn) => fn());
    this.unlistenFns = [];
  }
}

// Export singleton instance
export const tasksState = new TasksState();

/**
 * Helper to get task status display color
 */
export function getTaskStatusColor(status: string): string {
  switch (status) {
    case 'pending':
      return 'text-gray-500';
    case 'queued':
      return 'text-blue-500';
    case 'processing':
      return 'text-yellow-500';
    case 'paused':
      return 'text-orange-500';
    case 'complete':
      return 'text-green-500';
    case 'error':
      return 'text-red-500';
    case 'cancelled':
      return 'text-gray-400';
    default:
      return 'text-gray-500';
  }
}

/**
 * Helper to get task status display text
 */
export function getTaskStatusText(status: string): string {
  return status.charAt(0).toUpperCase() + status.slice(1);
}

/**
 * Helper to format task type
 */
export function formatTaskType(taskType: string): string {
  switch (taskType) {
    case 'thumbnail':
      return 'Thumbnail';
    case 'metadata':
      return 'Metadata';
    default:
      return taskType.charAt(0).toUpperCase() + taskType.slice(1);
  }
}
