export type ParsedKeyValueLabels =
  | { readonly ok: true; readonly value: Readonly<Record<string, string>> }
  | { readonly ok: false; readonly error: string };

export function parseKeyValueLabels(value: string): ParsedKeyValueLabels {
  const labels: Record<string, string> = {};
  for (const entry of splitFormEntries(value)) {
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

export function splitFormEntries(value: string): readonly string[] {
  return [...new Set(value
    .split(/[\n,]/)
    .map((entry) => entry.trim())
    .filter(Boolean))];
}

export function labelsToFormText(labels: Readonly<Record<string, string>>): string {
  return Object.entries(labels)
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([key, value]) => `${key}=${value}`)
    .join('\n');
}
