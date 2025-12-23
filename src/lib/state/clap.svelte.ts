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
	type SemanticSearchResult
} from '$lib/database/queries';

/**
 * CLAP state for server management and semantic search
 */
class ClapState {
	// Server status
	serverAvailable = $state(false);
	serverChecking = $state(false);
	serverStarting = $state(false);

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
