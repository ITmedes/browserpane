export type CertificateMetadata = {
  readonly spkiFingerprint: string | null;
  readonly certificateHash: string | null;
};

export type CertificateMetadataClientOptions = {
  readonly baseUrl: string | URL;
  readonly fetchImpl?: typeof fetch;
};

export class CertificateMetadataClient {
  readonly #baseUrl: URL;
  readonly #fetchImpl: typeof fetch;

  constructor(options: CertificateMetadataClientOptions) {
    this.#baseUrl = new URL(options.baseUrl);
    this.#fetchImpl = options.fetchImpl ?? fetch;
  }

  async load(): Promise<CertificateMetadata> {
    const [spkiFingerprint, certificateHash] = await Promise.all([
      this.#readOptionalText('/cert-fingerprint'),
      this.#readOptionalText('/cert-hash'),
    ]);
    return { spkiFingerprint, certificateHash };
  }

  async #readOptionalText(path: string): Promise<string | null> {
    const response = await this.#fetchImpl(new URL(path, this.#baseUrl));
    if (response.status === 404) {
      return null;
    }
    if (!response.ok) {
      throw new Error(`${path} request failed with HTTP ${response.status}`);
    }
    const value = (await response.text()).trim();
    return value.length > 0 ? value : null;
  }
}
