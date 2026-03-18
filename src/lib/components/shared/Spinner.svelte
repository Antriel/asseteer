<script lang="ts">
  interface Props {
    size?: 'sm' | 'md' | 'lg';
    color?: 'default' | 'blue';
  }

  let { size = 'md', color = 'default' }: Props = $props();

  // Size classes
  const sizeClasses = {
    sm: 'w-4 h-4 border-2',
    md: 'w-5 h-5 border-2',
    lg: 'w-10 h-10 border-3',
  };

  // Color classes
  const colorClasses = {
    default: 'border-default border-t-accent',
    blue: 'border-blue-500 border-t-transparent',
  };

  const classes = $derived(`${sizeClasses[size]} ${colorClasses[color]}`);
</script>

<div class="{classes} rounded-full animate-spin" role="status" aria-label="Loading"></div>
