/**
 * CLAP (semantic audio search) state management
 */

import {
	checkClapServer,
	startClapServer,
	processClapEmbeddings,
	getPendingClapCount,
	searchAudioSemantic,
	type SemanticSearchResult,
	type ProcessClapResult
} from '$lib/database/queries';

/**
 * CLAP state for semantic search and embedding processing
 */
class ClapState {
	// Server status
	serverAvailable = $state(false);
	serverChecking = $state(false);
	serverStarting = $state(false);

	// Embedding processing
	pendingCount = $state(0);
	isProcessing = $state(false);
	processProgress = $state<{ processed: number; total: number } | null>(null);
	lastProcessResult = $state<ProcessClapResult | null>(null);

	// Semantic search
	semanticSearchEnabled = $state(false);
	semanticResults = $state<SemanticSearchResult[]>([]);
	isSearching = $state(false);
	lastSearchQuery = $state('');

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
	 * Refresh pending embedding count
	 */
	async refreshPendingCount(): Promise<number> {
		try {
			this.pendingCount = await getPendingClapCount();
			return this.pendingCount;
		} catch (error) {
			console.error('[CLAP] Failed to get pending count:', error);
			return 0;
		}
	}

	/**
	 * Process CLAP embeddings for pending audio assets
	 */
	async processEmbeddings(batchSize?: number): Promise<ProcessClapResult | null> {
		if (this.isProcessing) {
			console.warn('[CLAP] Already processing');
			return null;
		}

		// Ensure server is running
		if (!(await this.ensureServer())) {
			throw new Error('CLAP server is not available');
		}

		this.isProcessing = true;
		this.processProgress = { processed: 0, total: this.pendingCount };

		try {
			const result = await processClapEmbeddings(batchSize);
			this.lastProcessResult = result;

			// Refresh pending count after processing
			await this.refreshPendingCount();

			return result;
		} catch (error) {
			console.error('[CLAP] Processing failed:', error);
			throw error;
		} finally {
			this.isProcessing = false;
			this.processProgress = null;
		}
	}

	/**
	 * Perform semantic search
	 */
	async search(query: string, limit: number = 50): Promise<SemanticSearchResult[]> {
		if (!query.trim()) {
			this.semanticResults = [];
			this.lastSearchQuery = '';
			return [];
		}

		// Ensure server is running
		if (!(await this.ensureServer())) {
			throw new Error('CLAP server is not available');
		}

		this.isSearching = true;
		this.lastSearchQuery = query;

		try {
			const results = await searchAudioSemantic(query, limit);
			this.semanticResults = results;
			return results;
		} catch (error) {
			console.error('[CLAP] Search failed:', error);
			this.semanticResults = [];
			throw error;
		} finally {
			this.isSearching = false;
		}
	}

	/**
	 * Clear semantic search results
	 */
	clearSearch() {
		this.semanticResults = [];
		this.lastSearchQuery = '';
		this.semanticSearchEnabled = false;
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
}

// Export singleton instance
export const clapState = new ClapState();

/**
 * Format similarity score as percentage
 */
export function formatSimilarity(similarity: number): string {
	return `${Math.round(similarity * 100)}%`;
}
