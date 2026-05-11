import { describe, expect, it, vi } from 'vitest';
import { BrowserSessionSdkLoader } from './browser-session-sdk-loader';
import type { BrowserSessionConnectOptions } from './browser-session-types';

describe('BrowserSessionSdkLoader', () => {
  it('cache-busts the default SDK import URL', async () => {
    const urls: string[] = [];
    class FakeBpaneSession {
      static async connect(): Promise<{ disconnect: () => void }> {
        return { disconnect: vi.fn() };
      }
    }
    const loader = new BrowserSessionSdkLoader({
      cacheToken: 'test-token',
      importer: async (moduleUrl) => {
        urls.push(moduleUrl);
        return { BpaneSession: FakeBpaneSession };
      },
    });

    await loader.load();

    expect(urls).toEqual(['/dist/bpane.js?bpane_admin_sdk=test-token']);
  });

  it('accepts the class-shaped BpaneSession export from the built SDK', async () => {
    const handle = { disconnect: vi.fn() };
    class FakeBpaneSession {
      static async connect(_options: BrowserSessionConnectOptions): Promise<typeof handle> {
        return handle;
      }
    }
    const loader = new BrowserSessionSdkLoader({
      importer: async () => ({ BpaneSession: FakeBpaneSession }),
    });

    const sdk = await loader.load();
    const session = await sdk.BpaneSession.connect({
      container: document.createElement('div'),
      gatewayUrl: 'https://localhost:4433/session',
      connectTicket: 'ticket',
    });

    expect(session).toBe(handle);
  });
});
