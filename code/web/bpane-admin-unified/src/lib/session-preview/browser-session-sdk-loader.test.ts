import { describe, expect, it, vi } from 'vitest';

import { BrowserSessionSdkLoader } from './browser-session-sdk-loader';

describe('BrowserSessionSdkLoader', () => {
  it('loads the BrowserPane SDK and validates the connect handle', async () => {
    const disconnect = vi.fn();
    const importer = vi.fn(async () => ({
      BpaneSession: {
        connect: vi.fn(async () => ({ disconnect })),
      },
    }));
    const loader = new BrowserSessionSdkLoader({
      moduleUrl: '/dist/bpane.js',
      importer,
      cacheBust: false,
    });

    const sdk = await loader.load();
    const handle = await sdk.BpaneSession.connect({
      container: document.createElement('div'),
      gatewayUrl: 'https://localhost:4433/session',
    });

    handle.disconnect();
    expect(disconnect).toHaveBeenCalledOnce();
    expect(importer).toHaveBeenCalledWith('/dist/bpane.js');
  });

  it('rejects SDK modules without BpaneSession.connect', async () => {
    const loader = new BrowserSessionSdkLoader({
      importer: async () => ({ BpaneSession: {} }),
    });

    await expect(loader.load()).rejects.toThrow('BpaneSession.connect');
  });
});
