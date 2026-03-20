import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { SvelteMap, SvelteSet } from 'svelte/reactivity';
import { getDatabase } from '$lib/database/connection';
import { getPendingAssetCounts, getPendingClapCount } from '$lib/database/queries';
import { settings } from '$lib/state/settings.svelte';
import type {
  PendingCount,
  ProcessingCategory,
  CategoryProgress,
  ProcessingErrorDetail,
} from '$lib/types';

/**
 * Category-aware processing state management
 *
 * Tracks processing progress independently for each category (image, audio, etc.)
 * and allows independent control of each category.
 */
export interface ProcessingRunResult {
  total: number;
  completed: number;
  failed: number;
}

class ProcessingState {
  // Per-category progress tracking (using SvelteMap for reactivity)
  categoryProgress = $state(new SvelteMap<ProcessingCategory, CategoryProgress>());

  // Categories enabled for processing (using SvelteSet for reactivity)
  enabledCategories = $state(new SvelteSet<ProcessingCategory>(['image', 'audio', 'clap']));

  // Pending asset count (from database)
  pendingCount = $state<PendingCount>({ images: 0, audio: 0, clap: 0, total: 0 });

  // Result from last completed processing run (shown in status bar until next run)
  lastRunResult = $state<ProcessingRunResult | null>(null);

  // Event listeners
  private unlistenFns: UnlistenFn[] = [];

  /**
   * Initialize event listeners for processing events
   */
  async initializeListeners() {
    console.time('[Processing] initializeListeners');
    // Clean up existing listeners
    this.cleanup();

    const categories: ProcessingCategory[] = ['image', 'audio', 'clap'];

    const listenerPromises = categories.flatMap((category) => [
      listen<CategoryProgress>(`processing-progress-${category}`, (event) => {
        console.log(`[Processing] ${category} progress:`, event.payload);
        this.updateCategoryProgress(category, event.payload);
      }),
      listen<CategoryProgress>(`processing-complete-${category}`, async (event) => {
        console.log(`[Processing] ${category} complete:`, event.payload);
        this.updateCategoryProgress(category, event.payload);
        await this.refreshPendingCount();
        this.checkAllComplete();
      }),
    ]);

    this.unlistenFns = await Promise.all(listenerPromises);
    console.timeEnd('[Processing] initializeListeners');
  }

  /**
   * Check if all categories have finished and update lastRunResult
   */
  private checkAllComplete() {
    let anyRunning = false;
    let total = 0;
    let completed = 0;
    let failed = 0;

    for (const progress of this.categoryProgress.values()) {
      if (progress.isRunning) {
        anyRunning = true;
        break;
      }
      total += progress.total;
      completed += progress.completed;
      failed += progress.failed;
    }

    if (!anyRunning && total > 0) {
      this.lastRunResult = { total, completed, failed };
      console.log('[Processing] All complete:', this.lastRunResult);
    }
  }

  /**
   * Update progress for a specific category
   */
  private updateCategoryProgress(category: ProcessingCategory, progress: CategoryProgress) {
    this.categoryProgress.set(category, {
      ...progress,
      isPaused: progress.is_paused,
      isRunning: progress.is_running,
    });
  }

  /**
   * Start processing for a specific category
   */
  async startProcessing(category: ProcessingCategory) {
    // Check if category has pending items
    const pendingCount = this.getPendingCountForCategory(category);
    if (pendingCount === 0) {
      console.log(`[Processing] Skipping ${category}: No pending assets`);
      return; // Gracefully skip instead of throwing error
    }

    // Clear last run result when starting new processing
    this.lastRunResult = null;

    try {
      await invoke('start_processing', {
        category,
        preGenerateThumbnails: settings.preGenerateThumbnails,
      });
      console.log(`[Processing] Started ${category}`);

      // Query initial progress
      await this.refreshProgress(category);
    } catch (error) {
      console.error(`[Processing] Failed to start ${category}:`, error);
      throw error;
    }
  }

  /**
   * Start processing for all enabled categories
   */
  async startAllEnabled() {
    const promises = Array.from(this.enabledCategories)
      .filter((category) => this.getPendingCountForCategory(category) > 0) // Only start categories with pending items
      .map((category) =>
        this.startProcessing(category).catch((error) => {
          console.error(`Failed to start ${category}:`, error);
          // Continue with other categories even if one fails
        }),
      );

    if (promises.length === 0) {
      console.log('[Processing] No enabled categories have pending assets');
      return;
    }

    await Promise.all(promises);
  }

  /**
   * Pause processing for a specific category
   */
  async pause(category: ProcessingCategory) {
    try {
      await invoke('pause_processing', { category });
      console.log(`[Processing] Paused ${category}`);
      // Backend will emit progress update with paused state
    } catch (error) {
      console.error(`[Processing] Failed to pause ${category}:`, error);
      throw error;
    }
  }

  /**
   * Resume processing for a specific category
   */
  async resume(category: ProcessingCategory) {
    try {
      await invoke('resume_processing', { category });
      console.log(`[Processing] Resumed ${category}`);
      // Backend will emit progress update with resumed state
    } catch (error) {
      console.error(`[Processing] Failed to resume ${category}:`, error);
      throw error;
    }
  }

  /**
   * Stop processing for a specific category
   */
  async stop(category: ProcessingCategory, skipPendingRefresh = false) {
    try {
      await invoke('stop_processing', { category });
      console.log(`[Processing] Stopped ${category}`);
      // State will be updated by backend or can be refreshed
      await this.refreshProgress(category);
      // Refresh pending count since stopped assets remain unprocessed
      if (!skipPendingRefresh) {
        await this.refreshPendingCount();
      }
    } catch (error) {
      console.error(`[Processing] Failed to stop ${category}:`, error);
      throw error;
    }
  }

  /**
   * Pause all running categories
   */
  async pauseAll() {
    const promises = Array.from(this.categoryProgress.entries())
      .filter(([_, progress]) => progress.isRunning && !progress.isPaused)
      .map(([category]) => this.pause(category).catch(console.error));

    await Promise.all(promises);
  }

  /**
   * Resume all paused categories
   */
  async resumeAll() {
    const promises = Array.from(this.categoryProgress.entries())
      .filter(([_, progress]) => progress.isRunning && progress.isPaused)
      .map(([category]) => this.resume(category).catch(console.error));

    await Promise.all(promises);
  }

  /**
   * Stop all running categories
   */
  async stopAll() {
    const promises = Array.from(this.categoryProgress.entries())
      .filter(([_, progress]) => progress.isRunning)
      .map(([category]) => this.stop(category, true).catch(console.error)); // Skip individual refreshes

    await Promise.all(promises);
    // Refresh pending count once after all categories stopped
    await this.refreshPendingCount();
  }

  /**
   * Refresh progress from backend for specific category or all categories
   */
  async refreshProgress(category?: ProcessingCategory) {
    try {
      const progressList = await invoke<CategoryProgress[]>('get_processing_progress', {
        category: category || null,
      });

      for (const progress of progressList) {
        const cat = progress.category as ProcessingCategory;
        this.updateCategoryProgress(cat, progress);
      }

      return progressList;
    } catch (error) {
      console.error('[Processing] Failed to refresh progress:', error);
      throw error;
    }
  }

  /**
   * Refresh pending asset count from database
   */
  async refreshPendingCount() {
    console.time('[Processing] refreshPendingCount');
    try {
      const db = await getDatabase();
      console.time('[Processing] getPendingCounts queries');
      const [assetCounts, clapCount] = await Promise.all([
        getPendingAssetCounts(db),
        getPendingClapCount(),
      ]);
      console.timeEnd('[Processing] getPendingCounts queries');
      this.pendingCount = {
        images: assetCounts.images,
        audio: assetCounts.audio,
        clap: clapCount,
        total: assetCounts.images + assetCounts.audio + clapCount,
      };
      console.log('[Processing] Pending count updated:', $state.snapshot(this.pendingCount));
      return this.pendingCount;
    } catch (error) {
      console.error('[Processing] Failed to refresh pending count:', error);
      throw error;
    } finally {
      console.timeEnd('[Processing] refreshPendingCount');
    }
  }

  /**
   * Toggle category enabled state
   */
  toggleCategory(category: ProcessingCategory) {
    if (this.enabledCategories.has(category)) {
      this.enabledCategories.delete(category);
    } else {
      this.enabledCategories.add(category);
    }
    // SvelteSet handles reactivity automatically, no need to reassign
  }

  /**
   * Get pending count for a specific category
   */
  getPendingCountForCategory(category: ProcessingCategory): number {
    if (category === 'image') return this.pendingCount.images;
    if (category === 'audio') return this.pendingCount.audio;
    if (category === 'clap') return this.pendingCount.clap;
    return 0;
  }

  /**
   * Fetch processing errors for a category
   */
  async fetchErrors(category?: ProcessingCategory): Promise<ProcessingErrorDetail[]> {
    try {
      const errors = await invoke<ProcessingErrorDetail[]>('get_processing_errors', {
        category: category || null,
      });
      return errors;
    } catch (error) {
      console.error('[Processing] Failed to fetch errors:', error);
      throw error;
    }
  }

  /**
   * Retry failed assets for a category
   */
  async retryFailed(category: ProcessingCategory): Promise<number> {
    try {
      const count = await invoke<number>('retry_failed_assets', { category });
      console.log(`[Processing] Retrying ${count} failed ${category} assets`);

      // Refresh progress after starting retry
      await this.refreshProgress(category);

      return count;
    } catch (error) {
      console.error(`[Processing] Failed to retry ${category}:`, error);
      throw error;
    }
  }

  /**
   * Clear processing errors
   */
  async clearErrors(category?: ProcessingCategory, onlyResolved = true): Promise<number> {
    try {
      const count = await invoke<number>('clear_processing_errors', {
        category: category || null,
        onlyResolved,
      });
      console.log(`[Processing] Cleared ${count} errors`);
      return count;
    } catch (error) {
      console.error('[Processing] Failed to clear errors:', error);
      throw error;
    }
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
 * Get overall progress across all categories
 */
export function getOverallProgress(state: ProcessingState): {
  total: number;
  completed: number;
  failed: number;
  percentage: number;
} {
  let total = 0;
  let completed = 0;
  let failed = 0;

  for (const progress of state.categoryProgress.values()) {
    total += progress.total;
    completed += progress.completed;
    failed += progress.failed;
  }

  const percentage = total === 0 ? 0 : Math.round(((completed + failed) / total) * 100);

  return { total, completed, failed, percentage };
}

/**
 * Get progress for a specific category
 */
export function getCategoryProgress(
  state: ProcessingState,
  category: ProcessingCategory,
): CategoryProgress | null {
  return state.categoryProgress.get(category) || null;
}

/**
 * Check if any category is running
 */
export function isAnyRunning(state: ProcessingState): boolean {
  for (const progress of state.categoryProgress.values()) {
    if (progress.isRunning) return true;
  }
  return false;
}

/**
 * Check if any category is paused
 */
export function isAnyPaused(state: ProcessingState): boolean {
  for (const progress of state.categoryProgress.values()) {
    if (progress.isPaused) return true;
  }
  return false;
}

/**
 * Get status text for display
 */
export function getStatusText(state: ProcessingState): string {
  const anyRunning = isAnyRunning(state);
  const anyPaused = isAnyPaused(state);

  if (!anyRunning) return 'Idle';
  if (anyPaused) return 'Paused';
  return 'Processing';
}

/**
 * Helper to get status display color
 */
export function getStatusColor(state: ProcessingState): string {
  const anyRunning = isAnyRunning(state);
  const anyPaused = isAnyPaused(state);
  const overall = getOverallProgress(state);

  if (!anyRunning) return 'text-gray-500';
  if (anyPaused) return 'text-orange-500';
  if (overall.failed > 0) return 'text-yellow-500';
  return 'text-blue-500';
}

/**
 * Check if a category can be started
 */
export function canStartCategory(state: ProcessingState, category: ProcessingCategory): boolean {
  const progress = state.categoryProgress.get(category);
  const pendingCount = state.getPendingCountForCategory(category);

  // Can start if: has pending items AND not currently running
  return pendingCount > 0 && (!progress || !progress.isRunning);
}

/**
 * Get status for a specific category
 */
export function getCategoryStatus(
  state: ProcessingState,
  category: ProcessingCategory,
): 'idle' | 'running' | 'paused' | 'completed' {
  const progress = state.categoryProgress.get(category);

  if (!progress || !progress.isRunning) {
    // Check if completed (all processed and no new pending assets)
    if (
      progress &&
      progress.total > 0 &&
      progress.completed + progress.failed === progress.total &&
      state.getPendingCountForCategory(category) === 0
    ) {
      return 'completed';
    }
    return 'idle';
  }

  if (progress.isPaused) return 'paused';
  return 'running';
}

/**
 * Format ETA seconds for display
 */
export function formatEta(seconds: number | null): string {
  if (seconds === null || seconds <= 0) return '--';

  if (seconds < 60) {
    return `${Math.round(seconds)}s`;
  } else if (seconds < 3600) {
    const mins = Math.floor(seconds / 60);
    const secs = Math.round(seconds % 60);
    return `${mins}m ${secs}s`;
  } else {
    const hours = Math.floor(seconds / 3600);
    const mins = Math.floor((seconds % 3600) / 60);
    return `${hours}h ${mins}m`;
  }
}

/**
 * Format processing rate for display
 */
export function formatRate(rate: number): string {
  if (rate <= 0) return '--';
  if (rate < 1) {
    return `${(rate * 60).toFixed(1)}/min`;
  }
  return `${rate.toFixed(1)}/s`;
}
