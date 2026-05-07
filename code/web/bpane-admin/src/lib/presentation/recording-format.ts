export function formatRecordingBytes(byteCount: number | null): string {
  if (!Number.isFinite(byteCount) || !byteCount || byteCount <= 0) {
    return '0 B';
  }
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let value = byteCount;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }
  return `${unitIndex === 0 ? value.toFixed(0) : value.toFixed(1)} ${units[unitIndex] ?? 'B'}`;
}

export function formatRecordingDuration(durationMs: number | null): string {
  if (!Number.isFinite(durationMs) || !durationMs || durationMs <= 0) {
    return '0s';
  }
  if (durationMs < 1000) {
    return `${durationMs}ms`;
  }
  return `${(durationMs / 1000).toFixed(1)}s`;
}

export function formatRecordingTimestamp(value: string | null): string {
  if (!value) {
    return '--';
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(date);
}

export function formatRecordingReason(value: string | null): string {
  return value ? value.replaceAll('_', ' ') : '--';
}

export function shortRecordingId(value: string): string {
  return value.length > 13 ? `${value.slice(0, 8)}...${value.slice(-4)}` : value;
}
