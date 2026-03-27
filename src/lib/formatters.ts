/**
 * UI Formatting Utilities
 * Used across pages for consistent data presentation.
 */

/**
 * Format bytes into human-readable size.
 * e.g. 75888123 → "75.9 MB", 186280000 → "186.3 MB"
 */
export function formatBytes(input: string | number): string {
  const num = typeof input === 'string' ? parseFloat(input) : input;
  if (isNaN(num) || num === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(num) / Math.log(k));
  return parseFloat((num / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
}

/**
 * Format a Docker size string (e.g. "75.89MB", "1.23GB") — pass through if already formatted.
 * If it's a raw number, convert to human-readable.
 */
export function formatSize(input: string): string {
  if (!input) return '—';
  // Already formatted (contains unit suffix)
  if (/[KMGT]B/i.test(input)) return input;
  // Raw number
  const num = parseFloat(input);
  if (!isNaN(num)) return formatBytes(num);
  return input;
}

/**
 * Format a timestamp into relative time or readable date.
 * Handles: Unix epoch (seconds or ms), ISO 8601, Docker "2026-03-25 03:20:44 +0700" format.
 */
export function formatTimestamp(input: string | number): string {
  if (!input || input === '0') return '—';

  let date: Date;

  if (typeof input === 'number' || /^\d{10,13}$/.test(String(input))) {
    // Unix epoch — seconds (10 digits) or milliseconds (13 digits)
    const num = typeof input === 'number' ? input : parseInt(input, 10);
    date = new Date(num > 1e12 ? num : num * 1000);
  } else {
    date = new Date(String(input));
  }

  if (isNaN(date.getTime())) return String(input);

  // Relative time
  const now = Date.now();
  const diff = now - date.getTime();
  const seconds = Math.floor(diff / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);

  if (days > 30) {
    return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' });
  }
  if (days > 0) return `${days}d ago`;
  if (hours > 0) return `${hours}h ago`;
  if (minutes > 0) return `${minutes}m ago`;
  return 'just now';
}

/**
 * Truncate a hash / long string with ellipsis, keeping first N chars.
 */
export function truncateId(id: string, length = 12): string {
  if (!id) return '';
  const clean = id.replace(/^sha256:/, '');
  return clean.length > length ? clean.substring(0, length) + '…' : clean;
}

/**
 * Truncate a volume name if it looks like a hash (>32 chars of hex).
 * Named volumes (short, readable) are kept as-is.
 */
export function formatVolumeName(name: string): { display: string; isHash: boolean } {
  if (!name) return { display: '', isHash: false };
  const isHash = /^[a-f0-9]{32,}$/i.test(name);
  if (isHash) {
    return { display: name.substring(0, 12) + '…', isHash: true };
  }
  return { display: name, isHash: false };
}
