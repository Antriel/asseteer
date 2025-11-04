import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getDatabase } from '$lib/database/connection';
import { getPendingAssetCounts } from '$lib/database/queries';
import type { PendingCount } from '$lib/types';

/**
 * Processing progress from backend
 */
export interface ProcessingProgress {
  total: number;
  completed: number;
  failed: number;
  is_paused: boolean;
  is_running: boolean;
}

/**
 * Simplified processing state management
 *
 * The new architecture treats all asset processing as a single logical task
 * with batch progress updates instead of tracking individual per-asset tasks.
 */
class ProcessingState {
  // Progress statistics
  total = $state(0);
  completed = $state(0);
  failed = $state(0);
  isPaused = $state(false);
  isRunning = $state(false);

  // Pending asset count (from database)
  pendingCount = $state<PendingCount>({ images: 0, audio: 0, total: 0 });

  // Event listeners
  private unlistenFns: UnlistenFn[] = [];

  /**
   * Initialize event listeners for processing events
   */
  async initializeListeners() {
    // Clean up existing listeners
    this.cleanup();

    // Listen for batch progress updates (every 10 assets or 2 seconds)
    const unlistenProgress = await listen<ProcessingProgress>('processing-progress', (event) => {
      console.log('[Processing] Progress update:', event.payload);
      this.updateProgress(event.payload);
    });

    // Listen for processing completion
    const unlistenComplete = await listen<ProcessingProgress>('processing-complete', async (event) => {
      console.log('[Processing] Complete:', event.payload);
      this.updateProgress(event.payload);
      // Refresh pending count after processing completes
      await this.refreshPendingCount();
    });

    this.unlistenFns = [unlistenProgress, unlistenComplete];
  }

  /**
   * Update progress from backend event
   */
  private updateProgress(progress: ProcessingProgress) {
    this.total = progress.total;
    this.completed = progress.completed;
    this.failed = progress.failed;
    this.isPaused = progress.is_paused;
    this.isRunning = progress.is_running;
  }

  /**
   * Start processing all pending assets
   */
  async startProcessing() {
    try {
      await invoke('start_processing_assets');
      console.log('[Processing] Started');

      // Query initial progress
      await this.refreshProgress();
    } catch (error) {
      console.error('[Processing] Failed to start:', error);
      throw error;
    }
  }

  /**
   * Pause processing
   */
  async pause() {
    try {
      await invoke('pause_processing');
      console.log('[Processing] Paused');
      // Backend will emit progress update with paused state
    } catch (error) {
      console.error('[Processing] Failed to pause:', error);
      throw error;
    }
  }

  /**
   * Resume processing
   */
  async resume() {
    try {
      await invoke('resume_processing');
      console.log('[Processing] Resumed');
      // Backend will emit progress update with resumed state
    } catch (error) {
      console.error('[Processing] Failed to resume:', error);
      throw error;
    }
  }

  /**
   * Stop processing completely
   */
  async stop() {
    try {
      await invoke('stop_processing');
      console.log('[Processing] Stopped');
      // State will be updated by backend or can be refreshed
      await this.refreshProgress();
    } catch (error) {
      console.error('[Processing] Failed to stop:', error);
      throw error;
    }
  }

  /**
   * Refresh progress from backend
   */
  async refreshProgress() {
    try {
      const progress = await invoke<ProcessingProgress>('get_processing_progress');
      this.updateProgress(progress);
      return progress;
    } catch (error) {
      console.error('[Processing] Failed to refresh progress:', error);
      throw error;
    }
  }

  /**
   * Refresh pending asset count from database
   */
  async refreshPendingCount() {
    try {
      const db = await getDatabase();
      const count = await getPendingAssetCounts(db);
      this.pendingCount = count;
      console.log('[Processing] Pending count updated:', count);
      return count;
    } catch (error) {
      console.error('[Processing] Failed to refresh pending count:', error);
      throw error;
    }
  }

  /**
   * Get progress percentage (0-100)
   */
  getProgressPercentage(): number {
    if (this.total === 0) return 0;
    return Math.round(((this.completed + this.failed) / this.total) * 100);
  }

  /**
   * Get status text for display
   */
  getStatusText(): string {
    if (!this.isRunning) return 'Idle';
    if (this.isPaused) return 'Paused';
    return 'Processing';
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
export const processingState = new ProcessingState();

/**
 * Helper to get status display color
 */
export function getStatusColor(state: ProcessingState): string {
  if (!state.isRunning) return 'text-gray-500';
  if (state.isPaused) return 'text-orange-500';
  if (state.failed > 0) return 'text-yellow-500';
  return 'text-blue-500';
}
