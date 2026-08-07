export type ParsedLabels =
  | { readonly ok: true; readonly value: Readonly<Record<string, string>> }
  | { readonly ok: false; readonly error: string };

export function parseIdentityLabels(value: string): ParsedLabels {
  const labels: Record<string, string> = {};
  for (const entry of splitIdentityList(value)) {
    const separator = entry.indexOf('=');
    if (separator <= 0) {
      return { ok: false, error: 'Labels must use key=value format.' };
    }
    const key = entry.slice(0, separator).trim();
    const labelValue = entry.slice(separator + 1).trim();
    if (!key || !labelValue) {
      return { ok: false, error: 'Labels must include a non-empty key and value.' };
    }
    labels[key] = labelValue;
  }
  return { ok: true, value: labels };
}

export function splitIdentityList(value: string): readonly string[] {
  return [...new Set(value
    .split(/[\n,]/)
    .map((entry) => entry.trim())
    .filter(Boolean))];
}

export function labelsToText(labels: Readonly<Record<string, string>>): string {
  return Object.entries(labels)
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([key, value]) => `${key}=${value}`)
    .join('\n');
}

export function shortIdentityId(value: string): string {
  return value.length > 16 ? `${value.slice(0, 8)}...${value.slice(-5)}` : value;
}
