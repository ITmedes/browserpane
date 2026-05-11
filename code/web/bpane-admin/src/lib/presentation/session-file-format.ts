export function formatSessionFileBytes(byteCount: number): string {
  if (!Number.isFinite(byteCount) || byteCount <= 0) {
    return '0 B';
  }
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let value = byteCount;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }
  const formatted = unitIndex === 0 ? value.toFixed(0) : value.toFixed(1);
  return `${formatted} ${units[unitIndex] ?? 'B'}`;
}

export function formatSessionFileTimestamp(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(date);
}

export function formatSessionFileSource(value: string): string {
  return value.replaceAll('_', ' ');
}

export function shortSessionFileDigest(value: string): string {
  return value.length > 16 ? `${value.slice(0, 16)}...` : value;
}
