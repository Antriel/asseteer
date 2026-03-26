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
  durationMs: number;
}

class ProcessingState {
  // Per-category progress tracking (using SvelteMap for reactivity)
  categoryProgress = $state(new SvelteMap<ProcessingCategory, CategoryProgress>());

  // Categories that have been asked to start and are waiting for backend confirmation
  startingCategories = $state(new SvelteSet<ProcessingCategory>());

  // Categories enabled for processing (using SvelteSet for reactivity)
  enabledCategories = $state(new SvelteSet<ProcessingCategory>(['image', 'audio', 'clap']));

  // Categories that have been asked to stop but are still winding down
  stoppingCategories = $state(new SvelteSet<ProcessingCategory>());

  // Categories where stop was requested while still in starting state —
  // startProcessing() will stop them as soon as the backend confirms startup
  pendingStopCategories = $state(new SvelteSet<ProcessingCategory>());

  // Pending asset count (from database)
  pendingCount = $state<PendingCount>({ images: 0, audio: 0, clap: 0, total: 0 });

  // Result from last completed processing run (shown in status bar until next run)
  lastRunResult = $state<ProcessingRunResult | null>(null);

  // Per-category elapsed time tracking
  categoryStartedAt = $state(new SvelteMap<ProcessingCategory, number>());
  categoryDurationMs = $state(new SvelteMap<ProcessingCategory, number>());

  // Overall processing start time (earliest running category)
  private processingStartedAt: number | null = null;

  // Event listeners
  private unlistenFns: UnlistenFn[] = [];

  /**
   * Initialize event listeners for processing events
   */
  async initializeListeners() {
    // Clean up existing listeners
    this.cleanup();

    const categories: ProcessingCategory[] = ['image', 'audio', 'clap'];

    const listenerPromises = categories.flatMap((category) => [
      listen<CategoryProgress>(`processing-progress-${category}`, (event) => {
        this.updateCategoryProgress(category, event.payload);
      }),
      listen<CategoryProgress>(`processing-complete-${category}`, async (event) => {
        this.updateCategoryProgress(category, event.payload);
        await this.refreshPendingCount();
        this.checkAllComplete();
      }),
    ]);

    this.unlistenFns = await Promise.all(listenerPromises);
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
      const durationMs = this.processingStartedAt
        ? Date.now() - this.processingStartedAt
        : 0;
      this.lastRunResult = { total, completed, failed, durationMs };
      this.processingStartedAt = null;
    }
  }

  /**
   * Update progress for a specific category
   */
  private updateCategoryProgress(category: ProcessingCategory, progress: CategoryProgress) {
    this.startingCategories.delete(category);
    this.categoryProgress.set(category, {
      ...progress,
      isPaused: progress.is_paused,
      isRunning: progress.is_running,
    });
    // When a category stops running, record its duration
    if (!progress.is_running) {
      this.stoppingCategories.delete(category);
      const startedAt = this.categoryStartedAt.get(category);
      if (startedAt) {
        this.categoryDurationMs.set(category, Date.now() - startedAt);
        this.categoryStartedAt.delete(category);
      }
    }
  }

  /**
   * Start processing for a specific category
   */
  async startProcessing(category: ProcessingCategory) {
    // Check if category has pending items
    const pendingCount = this.getPendingCountForCategory(category);
    if (pendingCount === 0) {
      return; // Gracefully skip instead of throwing error
    }

    // Clear last run result when starting new processing
    this.lastRunResult = null;

    // Track start time for this category
    const now = Date.now();
    this.categoryStartedAt.set(category, now);
    this.categoryDurationMs.delete(category);
    if (!this.processingStartedAt) {
      this.processingStartedAt = now;
    }

    const previousProgress = this.categoryProgress.get(category) ?? null;
    this.startingCategories.add(category);
    this.categoryProgress.set(category, {
      category,
      total: pendingCount,
      completed: 0,
      failed: 0,
      is_paused: false,
      is_running: false,
      current_file: null,
      processing_rate: 0,
      eta_seconds: null,
      isPaused: false,
      isRunning: false,
    });

    try {
      await invoke('start_processing', {
        category,
        preGenerateThumbnails: settings.preGenerateThumbnails,
      });

      // Query initial progress
      await this.refreshProgress(category);

      // If stop was requested while we were starting, stop immediately
      if (this.pendingStopCategories.has(category)) {
        this.pendingStopCategories.delete(category);
        await this.stop(category);
        return;
      }
    } catch (error) {
      this.startingCategories.delete(category);
      this.pendingStopCategories.delete(category);
      if (previousProgress) {
        this.categoryProgress.set(category, previousProgress);
      } else {
        this.categoryProgress.delete(category);
      }
      this.categoryStartedAt.delete(category);
      this.categoryDurationMs.delete(category);
      if (this.categoryStartedAt.size === 0) {
        this.processingStartedAt = null;
      }
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
    // If the category is still starting (backend hasn't confirmed running yet),
    // queue the stop for when startup completes rather than calling the backend
    if (this.startingCategories.has(category)) {
      this.pendingStopCategories.add(category);
      this.stoppingCategories.add(category);
      return;
    }

    this.stoppingCategories.add(category);
    try {
      await invoke('stop_processing', { category });
      // State will be updated by backend or can be refreshed
      await this.refreshProgress(category);
      // Refresh pending count since stopped assets remain unprocessed
      if (!skipPendingRefresh) {
        await this.refreshPendingCount();
      }
    } catch (error) {
      const msg = String(error);
      // Silently ignore if already stopped (e.g. rapid double-click)
      if (msg.includes('not running')) {
        this.stoppingCategories.delete(category);
        return;
      }
      console.error(`[Processing] Failed to stop ${category}:`, error);
      this.stoppingCategories.delete(category);
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
    const promises: Promise<void>[] = [];

    // Stop categories that are already running
    for (const [category, progress] of this.categoryProgress.entries()) {
      if (progress.isRunning) {
        promises.push(this.stop(category, true).catch(console.error) as Promise<void>);
      }
    }

    // Queue stop for categories still in starting state
    for (const category of this.startingCategories) {
      if (!this.pendingStopCategories.has(category)) {
        this.pendingStopCategories.add(category);
        this.stoppingCategories.add(category);
      }
    }

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
    try {
      const db = await getDatabase();
      const [assetCounts, clapCount] = await Promise.all([
        getPendingAssetCounts(db, settings.preGenerateThumbnails),
        getPendingClapCount(),
      ]);
      this.pendingCount = {
        images: assetCounts.images,
        audio: assetCounts.audio,
        clap: clapCount,
        total: assetCounts.images + assetCounts.audio + clapCount,
      };
      return this.pendingCount;
    } catch (error) {
      console.error('[Processing] Failed to refresh pending count:', error);
      throw error;
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
  return pendingCount > 0 && !state.startingCategories.has(category) && (!progress || !progress.isRunning);
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

export function isCategoryStarting(state: ProcessingState, category: ProcessingCategory): boolean {
  return state.startingCategories.has(category);
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
 * Format elapsed milliseconds for display (e.g., "2m 45s", "1h 23m")
 */
export function formatElapsed(ms: number): string {
  const seconds = Math.round(ms / 1000);
  if (seconds < 60) {
    return `${seconds}s`;
  } else if (seconds < 3600) {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
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
