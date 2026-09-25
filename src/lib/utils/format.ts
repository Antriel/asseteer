/**
 * Format milliseconds as a playback time string (e.g., "1:05.234" or "0:05")
 */
export function formatDuration(ms: number): string {
  const totalSeconds = ms / 1000;
  const minutes = Math.floor(totalSeconds / 60);
  if (totalSeconds < 10) {
    // Integer ms math: (4.3 - 4) * 1000 is 299.99… in floating point.
    const wholeMs = Math.floor(ms);
    const wholeSecs = Math.floor(wholeMs / 1000) % 60;
    const millis = wholeMs % 1000;
    return `${minutes}:${wholeSecs.toString().padStart(2, '0')}.${millis.toString().padStart(3, '0')}`;
  }
  const remainingSeconds = Math.floor(totalSeconds % 60);
  return `${minutes}:${remainingSeconds.toString().padStart(2, '0')}`;
}

/**
 * Format milliseconds for scanning a list of sounds: seconds with a unit under a minute
 * ("0.35 s", "3.00 s", "12.5 s"), m:ss from a minute up ("1:05").
 */
export function formatDurationCompact(ms: number): string {
  // Round first, then pick the bracket, so 9.996 s reads "10.0 s", not "10.00 s"
  const hundredths = Math.round(ms / 10);
  if (hundredths < 1000) return `${(hundredths / 100).toFixed(2)} s`;
  const tenths = Math.round(ms / 100);
  if (tenths < 600) return `${(tenths / 10).toFixed(1)} s`;
  const totalSeconds = Math.round(ms / 1000);
  const seconds = totalSeconds % 60;
  return `${Math.floor(totalSeconds / 60)}:${seconds.toString().padStart(2, '0')}`;
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
