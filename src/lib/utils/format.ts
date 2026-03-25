/**
 * Format milliseconds as a playback time string (e.g., "1:05.234" or "0:05")
 */
export function formatDuration(ms: number): string {
  const totalSeconds = ms / 1000;
  const minutes = Math.floor(totalSeconds / 60);
  if (totalSeconds < 10) {
    const secs = totalSeconds % 60;
    const wholeSecs = Math.floor(secs);
    const millis = Math.floor((secs - wholeSecs) * 1000);
    return `${minutes}:${wholeSecs.toString().padStart(2, '0')}.${millis.toString().padStart(3, '0')}`;
  }
  const remainingSeconds = Math.floor(totalSeconds % 60);
  return `${minutes}:${remainingSeconds.toString().padStart(2, '0')}`;
}

/**
 * Format bytes as a human-readable file size string
 */
export function formatFileSize(bytes: number): string {
  if (bytes < 1024) return bytes + ' B';
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
  if (bytes < 1024 * 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
  return (bytes / (1024 * 1024 * 1024)).toFixed(1) + ' GB';
}

/**
 * Format a similarity score (0–1) as a percentage string
 */
export function formatSimilarity(similarity: number): string {
  return `${Math.round(similarity * 100)}%`;
}
