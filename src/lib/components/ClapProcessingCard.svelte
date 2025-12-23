<script lang="ts">
	import { clapState, formatSimilarity } from '$lib/state/clap.svelte';
	import { showToast } from '$lib/state/ui.svelte';
	import { onMount } from 'svelte';
	import Spinner from './shared/Spinner.svelte';

	// Check server and pending count on mount
	onMount(async () => {
		await clapState.checkServer();
		await clapState.refreshPendingCount();
	});

	async function handleStartProcessing() {
		try {
			const result = await clapState.processEmbeddings();
			if (result) {
				if (result.failed > 0) {
					showToast(`Processed ${result.processed} files, ${result.failed} failed`, 'warning');
				} else {
					showToast(`Processed ${result.processed} audio files`, 'success');
				}
			}
		} catch (error) {
			showToast(`Processing failed: ${error}`, 'error');
		}
	}

	async function handleStartServer() {
		try {
			console.log('[CLAP UI] Starting server...');
			const success = await clapState.ensureServer();
			console.log('[CLAP UI] ensureServer returned:', success, 'serverAvailable:', clapState.serverAvailable);
			if (success) {
				showToast('CLAP server started', 'success');
				// Refresh pending count now that server is available
				await clapState.refreshPendingCount();
			} else {
				showToast('Failed to start CLAP server', 'error');
			}
		} catch (error) {
			console.error('[CLAP UI] Error starting server:', error);
			showToast(`Failed to start server: ${error}`, 'error');
		}
	}

	// Derived states
	let canProcess = $derived(
		clapState.serverAvailable && clapState.pendingCount > 0 && !clapState.isProcessing
	);
	let statusText = $derived.by(() => {
		if (clapState.isProcessing) return 'Processing...';
		if (clapState.serverStarting) return 'Starting server...';
		if (!clapState.serverAvailable) return 'Server offline';
		if (clapState.pendingCount === 0) return 'Ready';
		return 'Ready';
	});
	let statusColor = $derived.by(() => {
		if (clapState.isProcessing) return 'text-blue-500';
		if (!clapState.serverAvailable) return 'text-gray-500';
		if (clapState.pendingCount === 0) return 'text-green-500';
		return 'text-orange-500';
	});
</script>

<div class="p-4 bg-primary border border-default rounded-lg">
	<div class="flex items-center justify-between mb-3">
		<div class="flex items-center gap-3">
			<!-- Audio wave icon -->
			<div class="w-8 h-8 flex items-center justify-center bg-purple-100 dark:bg-purple-900/30 rounded">
				<svg class="w-5 h-5 text-purple-600 dark:text-purple-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 19V6l12-3v13M9 19c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zm12-3c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zM9 10l12-3" />
				</svg>
			</div>
			<div>
				<h4 class="font-medium text-primary">Audio Embeddings (CLAP)</h4>
				<p class="text-xs text-secondary">Enables semantic audio search</p>
			</div>
		</div>

		<!-- Status badge -->
		<span class="px-2 py-1 text-xs font-medium rounded {statusColor} bg-opacity-10">
			{statusText}
		</span>
	</div>

	<!-- Server status and controls -->
	{#if !clapState.serverAvailable && !clapState.serverStarting}
		<div class="flex items-center justify-between p-3 bg-secondary rounded mb-3">
			<span class="text-sm text-secondary">CLAP server not running</span>
			<button
				onclick={handleStartServer}
				class="px-3 py-1.5 text-sm font-medium text-white bg-purple-500 hover:bg-purple-600 rounded transition-colors"
			>
				Start Server
			</button>
		</div>
	{:else if clapState.serverStarting}
		<div class="flex items-center gap-2 p-3 bg-secondary rounded mb-3">
			<Spinner size="sm" />
			<span class="text-sm text-secondary">Starting CLAP server (loading model)...</span>
		</div>
	{/if}

	<!-- Pending count and process button -->
	{#if clapState.serverAvailable}
		<div class="flex items-center justify-between">
			<div class="text-sm">
				{#if clapState.pendingCount > 0}
					<span class="text-orange-600 dark:text-orange-400 font-medium">
						{clapState.pendingCount} audio files
					</span>
					<span class="text-secondary"> need embeddings</span>
				{:else}
					<span class="text-green-600 dark:text-green-400">All audio files have embeddings</span>
				{/if}
			</div>

			{#if clapState.isProcessing}
				<div class="flex items-center gap-2">
					<Spinner size="sm" />
					<span class="text-sm text-secondary">Processing...</span>
				</div>
			{:else if clapState.pendingCount > 0}
				<button
					onclick={handleStartProcessing}
					disabled={!canProcess}
					class="px-3 py-1.5 text-sm font-medium text-white bg-purple-500 hover:bg-purple-600 disabled:bg-gray-400 disabled:cursor-not-allowed rounded transition-colors"
				>
					Generate Embeddings
				</button>
			{/if}
		</div>
	{/if}

	<!-- Last result info -->
	{#if clapState.lastProcessResult && !clapState.isProcessing}
		<div class="mt-3 pt-3 border-t border-default text-xs text-secondary">
			Last run: {clapState.lastProcessResult.processed} processed
			{#if clapState.lastProcessResult.failed > 0}
				, <span class="text-red-500">{clapState.lastProcessResult.failed} failed</span>
			{/if}
		</div>
	{/if}
</div>
