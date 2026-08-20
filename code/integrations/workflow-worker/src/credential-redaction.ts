export type GeneratedTotp = {
  readonly code: string;
  readonly digits: number;
  readonly periodSec: number;
  readonly generatedAt: number;
  readonly expiresAt: number;
};

export class CredentialRedactor {
  readonly #values = new Set<string>();
  readonly #totpDigits = new Set<number>();

  addPayload(value: unknown): void {
    this.#collectStrings(value);
  }

  addTotpDigits(digits: number | null | undefined): void {
    if (Number.isInteger(digits) && digits !== undefined && digits !== null && digits >= 6 && digits <= 8) {
      this.#totpDigits.add(digits);
    }
  }

  redactText(value: string): string {
    let redacted = value;
    const secrets = [...this.#values].sort((left, right) => right.length - left.length);
    for (const secret of secrets) {
      if (secret.length >= 3) {
        redacted = redacted.split(secret).join('[REDACTED]');
        continue;
      }
      const escaped = secret.replace(/[.*+?^${}()|[\]\\]/gu, '\\$&');
      redacted = redacted.replace(
        new RegExp(`(?<![A-Za-z0-9])${escaped}(?![A-Za-z0-9])`, 'gu'),
        '[REDACTED]',
      );
    }
    for (const digits of this.#totpDigits) {
      redacted = redacted.replace(
        new RegExp(`(?<![0-9])[0-9]{${digits}}(?![0-9])`, 'gu'),
        '[REDACTED]',
      );
    }
    return redacted;
  }

  redactValue(value: unknown): unknown {
    if (typeof value === 'string') {
      return this.redactText(value);
    }
    if (Array.isArray(value)) {
      return value.map((item) => this.redactValue(item));
    }
    if (value && typeof value === 'object') {
      return Object.fromEntries(
        Object.entries(value).map(([key, item]) => [key, this.redactValue(item)]),
      );
    }
    return value;
  }

  #collectStrings(value: unknown): void {
    if (typeof value === 'string') {
      if (value.length > 0) {
        this.#values.add(value);
      }
      return;
    }
    if (Array.isArray(value)) {
      for (const item of value) {
        this.#collectStrings(item);
      }
      return;
    }
    if (value && typeof value === 'object') {
      for (const item of Object.values(value)) {
        this.#collectStrings(item);
      }
    }
  }
}
