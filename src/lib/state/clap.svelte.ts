/**
 * CLAP (semantic audio search) state management
 *
 * Handles CLAP server management and semantic search.
 * Embedding processing is handled by the unified task system (tasks.svelte.ts).
 */

import {
	checkClapServer,
	startClapServer,
	searchAudioSemantic,
	getClapServerInfo,
	getClapCacheSize,
	clearClapCache,
	type SemanticSearchResult,
	type ClapServerInfo
} from '$lib/database/queries';
import type { DurationFilter } from '$lib/state/assets.svelte';

// Maximum number of semantic search results to display
const MAX_SEMANTIC_RESULTS = 500;

/**
 * CLAP state for server management and semantic search
 */
export type ClapSetupStatus = 'not-configured' | 'setting-up' | 'ready' | 'offline' | 'error';

class ClapState {
	// Server status
	serverAvailable = $state(false);
	serverChecking = $state(false);
	serverStarting = $state(false);

	// Server info (from detailed health check)
	device = $state<string | null>(null);
	model = $state<string | null>(null);

	// Setup
	setupStatus = $state<ClapSetupStatus>('not-configured');
	setupError = $state<string | null>(null);
	cacheSize = $state(0);

	// Semantic search
	semanticSearchEnabled = $state(false);
	semanticResults = $state<SemanticSearchResult[]>([]);
	isSearching = $state(false);
	lastSearchQuery = $state('');
	hasMoreResults = $state(false);

	// Health monitoring
	private healthInterval: ReturnType<typeof setInterval> | null = null;

	// Search cancellation tracking
	private searchVersion = 0;

	/**
	 * Check if CLAP server is available
	 */
	async checkServer(): Promise<boolean> {
		this.serverChecking = true;
		try {
			this.serverAvailable = await checkClapServer();
			return this.serverAvailable;
		} catch (error) {
			console.error('[CLAP] Server check failed:', error);
			this.serverAvailable = false;
			return false;
		} finally {
			this.serverChecking = false;
		}
	}

	/**
	 * Start CLAP server if not running
	 */
	async ensureServer(): Promise<boolean> {
		console.log('[CLAP State] ensureServer called, serverAvailable:', this.serverAvailable);
		if (this.serverAvailable) return true;

		this.serverStarting = true;
		try {
			console.log('[CLAP State] Calling startClapServer...');
			await startClapServer();
			console.log('[CLAP State] startClapServer returned successfully');
			this.serverAvailable = true;
			return true;
		} catch (error) {
			console.error('[CLAP State] Failed to start server:', error);
			this.serverAvailable = false;
			return false;
		} finally {
			this.serverStarting = false;
			console.log('[CLAP State] ensureServer finished, serverAvailable:', this.serverAvailable);
		}
	}

	/**
	 * Perform semantic search with cancellation support
	 */
	async search(query: string, limit: number = MAX_SEMANTIC_RESULTS, durationFilter?: DurationFilter): Promise<SemanticSearchResult[]> {
		// Increment version to cancel any in-progress search
		const currentVersion = ++this.searchVersion;

		if (!query.trim()) {
			this.semanticResults = [];
			this.lastSearchQuery = '';
			this.isSearching = false;
			this.hasMoreResults = false;
			return [];
		}

		// Clear previous results and show loading state immediately
		this.semanticResults = [];
		this.isSearching = true;
		this.lastSearchQuery = query;
		this.hasMoreResults = false;

		// Ensure server is running
		if (!(await this.ensureServer())) {
			// Check if this search was cancelled
			if (currentVersion !== this.searchVersion) {
				return [];
			}
			this.isSearching = false;
			throw new Error('CLAP server is not available');
		}

		// Check if cancelled during server startup
		if (currentVersion !== this.searchVersion) {
			return [];
		}

		try {
			// Request one extra to detect if there are more results
			const results = await searchAudioSemantic(query, limit + 1, durationFilter);

			// Only update results if this search is still current
			if (currentVersion === this.searchVersion) {
				this.hasMoreResults = results.length > limit;
				this.semanticResults = results.slice(0, limit);
				this.isSearching = false;
				return this.semanticResults;
			}
			// Search was cancelled, don't update state
			return [];
		} catch (error) {
			// Only update state if this search is still current
			if (currentVersion === this.searchVersion) {
				console.error('[CLAP] Search failed:', error);
				this.semanticResults = [];
				this.isSearching = false;
				throw error;
			}
			return [];
		}
	}

	/**
	 * Clear semantic search results
	 */
	clearSearch() {
		this.semanticResults = [];
		this.lastSearchQuery = '';
		this.semanticSearchEnabled = false;
		this.hasMoreResults = false;
	}

	/**
	 * Toggle semantic search mode
	 */
	toggleSemanticSearch() {
		this.semanticSearchEnabled = !this.semanticSearchEnabled;
		if (!this.semanticSearchEnabled) {
			this.clearSearch();
		}
	}

	/**
	 * Run first-time setup: start server (triggers uv download + Python install + model download)
	 */
	async setup(): Promise<boolean> {
		this.setupStatus = 'setting-up';
		this.setupError = null;
		try {
			await startClapServer();
			this.serverAvailable = true;
			await this.refreshServerInfo();
			this.setupStatus = 'ready';
			this.startHealthMonitor();
			return true;
		} catch (error) {
			const msg = error instanceof Error ? error.message : String(error);
			this.setupError = msg;
			this.setupStatus = 'error';
			return false;
		}
	}

	/**
	 * Fetch detailed server info (device, model) and update state
	 */
	async refreshServerInfo(): Promise<void> {
		try {
			const info: ClapServerInfo = await getClapServerInfo();
			this.device = info.device;
			this.model = info.model;
			this.setupStatus = 'ready';
		} catch {
			this.device = null;
			this.model = null;
		}
	}

	/**
	 * Refresh the cache size display
	 */
	async refreshCacheSize(): Promise<void> {
		try {
			this.cacheSize = await getClapCacheSize();
		} catch {
			this.cacheSize = 0;
		}
	}

	/**
	 * Clear the CLAP/uv cache and reset state
	 */
	async clearCache(): Promise<void> {
		await clearClapCache();
		this.cacheSize = 0;
		this.setupStatus = 'not-configured';
		this.serverAvailable = false;
		this.device = null;
		this.model = null;
	}

	/**
	 * Start periodic health monitoring (every 30s)
	 */
	startHealthMonitor() {
		this.stopHealthMonitor();
		this.healthInterval = setInterval(async () => {
			if (this.setupStatus === 'ready' || this.serverAvailable) {
				const ok = await checkClapServer();
				if (!ok) {
					this.serverAvailable = false;
					this.device = null;
					this.setupStatus = 'offline';
				}
			}
		}, 30_000);
	}

	/**
	 * Stop health monitoring
	 */
	stopHealthMonitor() {
		if (this.healthInterval) {
			clearInterval(this.healthInterval);
			this.healthInterval = null;
		}
	}

	/**
	 * Initialize: check if server is already running, update state accordingly
	 */
	async initialize(): Promise<void> {
		const available = await this.checkServer();
		if (available) {
			await this.refreshServerInfo();
			this.setupStatus = 'ready';
			this.startHealthMonitor();
		}
		await this.refreshCacheSize();
	}
}

// Export singleton instance
export const clapState = new ClapState();

/**
 * Format similarity score as percentage
 */
export function formatSimilarity(similarity: number): string {
	return `${Math.round(similarity * 100)}%`;
}
