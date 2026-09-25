<script lang="ts">
  interface Props {
    size?: 'sm' | 'md' | 'lg';
    class?: string;
    circled?: boolean;
  }

  let { size = 'md', class: className = '', circled = false }: Props = $props();

  const sizeClasses = {
    sm: 'w-4 h-4',
    md: 'w-5 h-5',
    lg: 'w-6 h-6',
  };
</script>

<svg class="{sizeClasses[size]} {className}" fill="currentColor" viewBox="0 0 20 20">
  {#if circled}
    <path
      fill-rule="evenodd"
      d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zM7 8a1 1 0 012 0v4a1 1 0 11-2 0V8zm5-1a1 1 0 00-1 1v4a1 1 0 102 0V8a1 1 0 00-1-1z"
      clip-rule="evenodd"
    />
  {:else}
    <path
      d="M5.75 3a.75.75 0 00-.75.75v12.5c0 .414.336.75.75.75h1.5a.75.75 0 00.75-.75V3.75A.75.75 0 007.25 3h-1.5zM12.75 3a.75.75 0 00-.75.75v12.5c0 .414.336.75.75.75h1.5a.75.75 0 00.75-.75V3.75a.75.75 0 00-.75-.75h-1.5z"
    />
  {/if}
</svg>
