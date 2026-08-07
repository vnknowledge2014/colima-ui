import { describe, it, expect } from 'vitest';
import { formatBytes, formatSize, formatTimestamp, truncateId, formatVolumeName } from './formatters';

describe('formatBytes', () => {
  it('returns 0 B for zero', () => {
    expect(formatBytes(0)).toBe('0 B');
  });

  it('formats bytes', () => {
    expect(formatBytes(512)).toBe('512 B');
  });

  it('formats kilobytes', () => {
    expect(formatBytes(1024)).toBe('1 KB');
  });

  it('formats megabytes', () => {
    expect(formatBytes(1048576)).toBe('1 MB');
  });

  it('formats gigabytes', () => {
    expect(formatBytes(1073741824)).toBe('1 GB');
  });

  it('formats fractional values with 1 decimal', () => {
    expect(formatBytes(75888123)).toBe('72.4 MB');
  });

  it('handles string input', () => {
    expect(formatBytes('2097152')).toBe('2 MB');
  });

  it('returns 0 B for NaN string', () => {
    expect(formatBytes('not-a-number')).toBe('0 B');
  });
});

describe('formatSize', () => {
  it('returns em dash for empty string', () => {
    expect(formatSize('')).toBe('—');
  });

  it('passes through already-formatted strings with unit', () => {
    expect(formatSize('75.89MB')).toBe('75.89MB');
    expect(formatSize('1.23GB')).toBe('1.23GB');
  });

  it('converts raw numeric string to human-readable', () => {
    expect(formatSize('1048576')).toBe('1 MB');
  });
});

describe('formatTimestamp', () => {
  it('returns em dash for falsy input', () => {
    expect(formatTimestamp('')).toBe('—');
    expect(formatTimestamp('0')).toBe('—');
  });

  it('formats "just now" for very recent timestamps', () => {
    const nowSeconds = Math.floor(Date.now() / 1000);
    expect(formatTimestamp(nowSeconds)).toBe('just now');
  });

  it('formats minutes ago', () => {
    const fiveMinutesAgo = Math.floor(Date.now() / 1000) - 300;
    expect(formatTimestamp(fiveMinutesAgo)).toBe('5m ago');
  });

  it('formats hours ago', () => {
    const twoHoursAgo = Math.floor(Date.now() / 1000) - 7200;
    expect(formatTimestamp(twoHoursAgo)).toBe('2h ago');
  });

  it('formats days ago', () => {
    const threeDaysAgo = Math.floor(Date.now() / 1000) - 86400 * 3;
    expect(formatTimestamp(threeDaysAgo)).toBe('3d ago');
  });

  it('formats as locale date for old timestamps', () => {
    const oldTimestamp = Math.floor(Date.now() / 1000) - 86400 * 40;
    const result = formatTimestamp(oldTimestamp);
    // Should look like a readable date (e.g. "Jun 25, 2026")
    expect(result).toMatch(/\w+ \d+, \d{4}/);
  });

  it('handles millisecond timestamps (13 digits)', () => {
    const nowMs = Date.now();
    expect(formatTimestamp(nowMs)).toBe('just now');
  });
});

describe('truncateId', () => {
  it('returns empty string for falsy input', () => {
    expect(truncateId('')).toBe('');
  });

  it('strips sha256: prefix', () => {
    const id = 'sha256:abc1234567890abcdef';
    // After stripping "sha256:", we get "abc1234567890abcdef" (19 chars).
    // Default truncation length is 12, so it gets truncated with '…'
    expect(truncateId(id)).toBe('abc123456789…');
  });

  it('truncates to default 12 chars', () => {
    expect(truncateId('abcdefghijklmnopqrstuvwxyz')).toBe('abcdefghijkl…');
  });

  it('truncates to custom length', () => {
    expect(truncateId('abcdefghijklmnopqrstuvwxyz', 6)).toBe('abcdef…');
  });

  it('does not truncate short IDs', () => {
    expect(truncateId('abc123', 12)).toBe('abc123');
  });
});

describe('formatVolumeName', () => {
  it('returns empty result for empty string', () => {
    const result = formatVolumeName('');
    expect(result.display).toBe('');
    expect(result.isHash).toBe(false);
  });

  it('detects hash-like names and truncates them', () => {
    const hash = 'a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4'; // 32 hex chars
    const result = formatVolumeName(hash);
    expect(result.isHash).toBe(true);
    expect(result.display).toBe('a1b2c3d4e5f6…');
  });

  it('keeps readable named volumes as-is', () => {
    const result = formatVolumeName('my-postgres-data');
    expect(result.isHash).toBe(false);
    expect(result.display).toBe('my-postgres-data');
  });

  it('treats short hex strings as named volumes', () => {
    // Less than 32 hex chars — not a hash
    const result = formatVolumeName('deadbeef');
    expect(result.isHash).toBe(false);
    expect(result.display).toBe('deadbeef');
  });
});
