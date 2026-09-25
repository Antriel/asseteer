<script lang="ts">
  interface Props {
    size?: 'sm' | 'md' | 'lg';
    class?: string;
  }

  let { size = 'md', class: className = '' }: Props = $props();

  const sizeClasses = {
    sm: 'w-4 h-4',
    md: 'w-5 h-5',
    lg: 'w-6 h-6',
  };
</script>

<svg
  class="{sizeClasses[size]} {className}"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
  viewBox="0 0 24 24"
>
  <path d="M5 4.5v15l10-7.5z" />
  <path d="M19 5v14" />
</svg>
