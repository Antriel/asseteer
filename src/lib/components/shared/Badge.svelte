<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		variant?: 'count' | 'info';
		children: Snippet;
	}

	let { variant = 'count', children }: Props = $props();

	// Variant classes
	const variantClasses = {
		count: 'text-xs px-2 py-0.5 bg-tertiary rounded-full',
		info: 'text-xs px-1.5 py-0.5 rounded bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300'
	};

	const classes = $derived(variantClasses[variant]);
</script>

<span class={classes}>
	{@render children()}
</span>
