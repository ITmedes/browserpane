import { describe, expect, it, vi } from 'vitest';
import { CertificateMetadataClient } from './certificate-metadata';

describe('CertificateMetadataClient', () => {
  it('loads SPKI fingerprint and certificate hash from local compose endpoints', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input) => {
      const url = input instanceof URL ? input.pathname : new URL(String(input)).pathname;
      if (url === '/cert-fingerprint') {
        return new Response('spki-value\n', { status: 200 });
      }
      if (url === '/cert-hash') {
        return new Response('hash-value\n', { status: 200 });
      }
      return new Response('', { status: 404 });
    });
    const client = new CertificateMetadataClient({
      baseUrl: 'http://localhost:8080/admin/',
      fetchImpl,
    });

    await expect(client.load()).resolves.toEqual({
      spkiFingerprint: 'spki-value',
      certificateHash: 'hash-value',
    });
  });

  it('uses null values when local certificate metadata is unavailable', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () => new Response('', { status: 404 }));
    const client = new CertificateMetadataClient({
      baseUrl: 'http://localhost:8080',
      fetchImpl,
    });

    await expect(client.load()).resolves.toEqual({
      spkiFingerprint: null,
      certificateHash: null,
    });
  });
});
