<script lang="ts">
  interface Props {
    size?: 'sm' | 'md' | 'lg';
    class?: string;
    direction?: 'up' | 'down' | 'left' | 'right';
  }

  let { size = 'md', class: className = '', direction = 'right' }: Props = $props();

  const sizeClasses = {
    sm: 'w-4 h-4',
    md: 'w-5 h-5',
    lg: 'w-6 h-6',
  };

  const rotations = {
    up: 'rotate-180',
    down: 'rotate-0',
    left: 'rotate-90',
    right: '-rotate-90',
  };
</script>

<svg
  class="{sizeClasses[size]} {rotations[direction]} {className}"
  fill="none"
  stroke="currentColor"
  viewBox="0 0 24 24"
>
  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
</svg>
