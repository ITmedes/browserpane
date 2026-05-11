export function expectRecord(value: unknown, label: string): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${label} must be an object`);
  }
  return value as Record<string, unknown>;
}

export function expectString(value: unknown, label: string): string {
  if (typeof value !== 'string' || value.length === 0) {
    throw new Error(`${label} must be a non-empty string`);
  }
  return value;
}

export function expectNumber(value: unknown, label: string): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    throw new Error(`${label} must be a finite number`);
  }
  return value;
}

export function expectBoolean(value: unknown, label: string): boolean {
  if (typeof value !== 'boolean') {
    throw new Error(`${label} must be a boolean`);
  }
  return value;
}

export function expectStringRecord(value: unknown, label: string): Readonly<Record<string, string>> {
  const object = expectRecord(value, label);
  return Object.fromEntries(
    Object.entries(object).map(([key, entry]) => [
      key,
      expectString(entry, `${label} ${key}`),
    ]),
  );
}

export function optionalString(value: unknown, label: string): string | null | undefined {
  if (value === undefined || value === null) {
    return value;
  }
  return expectString(value, label);
}
