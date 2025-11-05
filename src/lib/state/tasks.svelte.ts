import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getDatabase } from '$lib/database/connection';
import { getPendingAssetCounts } from '$lib/database/queries';
import type { PendingCount, ProcessingCategory, CategoryProgress } from '$lib/types';

/**
 * Category-aware processing state management
 *
 * Tracks processing progress independently for each category (image, audio, etc.)
 * and allows independent control of each category.
 */
class ProcessingState {
  // Per-category progress tracking
  categoryProgress = $state(new Map<ProcessingCategory, CategoryProgress>());

  // Categories enabled for processing
  enabledCategories = $state(new Set<ProcessingCategory>(['image', 'audio']));

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

    const categories: ProcessingCategory[] = ['image', 'audio'];

    for (const category of categories) {
      // Listen for category-specific progress updates
      const unlistenProgress = await listen<CategoryProgress>(
        `processing-progress-${category}`,
        (event) => {
          console.log(`[Processing] ${category} progress:`, event.payload);
          this.updateCategoryProgress(category, event.payload);
        }
      );

      // Listen for category-specific completion
      const unlistenComplete = await listen<CategoryProgress>(
        `processing-complete-${category}`,
        async (event) => {
          console.log(`[Processing] ${category} complete:`, event.payload);
          this.updateCategoryProgress(category, event.payload);
          // Refresh pending count after processing completes
          await this.refreshPendingCount();
        }
      );

      this.unlistenFns.push(unlistenProgress, unlistenComplete);
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
    try {
      await invoke('start_processing', { category });
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
    const promises = Array.from(this.enabledCategories).map((category) =>
      this.startProcessing(category).catch((error) => {
        console.error(`Failed to start ${category}:`, error);
        // Continue with other categories even if one fails
      })
    );

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
  async stop(category: ProcessingCategory) {
    try {
      await invoke('stop_processing', { category });
      console.log(`[Processing] Stopped ${category}`);
      // State will be updated by backend or can be refreshed
      await this.refreshProgress(category);
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
      .map(([category]) => this.stop(category).catch(console.error));

    await Promise.all(promises);
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
   * Toggle category enabled state
   */
  toggleCategory(category: ProcessingCategory) {
    if (this.enabledCategories.has(category)) {
      this.enabledCategories.delete(category);
    } else {
      this.enabledCategories.add(category);
    }
    // Force reactivity update
    this.enabledCategories = new Set(this.enabledCategories);
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
  category: ProcessingCategory
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
