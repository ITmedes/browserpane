import { describe, expect, it, vi } from 'vitest';

import { sessionResource } from '$lib/test-utils/session-fixtures';
import type { BrowserSessionConnectOptions } from './browser-session-types';
import { SessionPreviewConnector } from './session-preview-connector';

describe('SessionPreviewConnector', () => {
  it('mints a connect ticket and mounts the BrowserPane SDK in the popup container', async () => {
    let connectOptions: BrowserSessionConnectOptions | null = null;
    const handle = { disconnect: vi.fn() };
    const container = document.createElement('div');
    container.append(document.createElement('span'));
    container.setAttribute('style', 'width: 10px');
    const connector = new SessionPreviewConnector({
      sessionClient: {
        issueSessionAccessToken: async () => sessionAccessTokenPayload(),
      },
      sdkProvider: {
        load: async () => ({
          BpaneSession: {
            connect: async (options) => {
              connectOptions = options;
              return handle;
            },
          },
        }),
      },
    });

    const connection = await connector.connect(sessionResource(), container);

    expect(connection).toMatchObject({
      sessionId: 'session-1',
      gatewayUrl: 'https://localhost:4433/session/session-1',
      handle,
    });
    expect(container.childElementCount).toBe(0);
    expect(container.getAttribute('style')).toBeNull();
    expect(connectOptions).toMatchObject({
      container,
      gatewayUrl: 'https://localhost:4433/session/session-1',
      connectTicket: 'connect-ticket',
      clientRole: 'interactive',
      certHashUrl: '/cert-hash',
      fileTransfer: true,
    });
  });

  it('enriches WebTransport opening handshake failures', async () => {
    const connector = new SessionPreviewConnector({
      sessionClient: {
        issueSessionAccessToken: async () => sessionAccessTokenPayload(),
      },
      sdkProvider: {
        load: async () => ({
          BpaneSession: {
            connect: async () => {
              throw new Error('Opening handshake failed.');
            },
          },
        }),
      },
    });

    await expect(connector.connect(sessionResource(), document.createElement('div'))).rejects.toThrow(
      'trust the dev SPKI',
    );
  });
});

function sessionAccessTokenPayload() {
  return {
    session_id: 'session-1',
    token_type: 'session_connect_ticket',
    token: 'connect-ticket',
    expires_at: '2026-06-21T10:10:00.000Z',
    connect: {
      gateway_url: 'https://localhost:4433',
      transport_path: '/session/session-1',
      auth_type: 'session_connect_ticket',
      ticket_path: '/api/v1/sessions/session-1/access-tokens',
      compatibility_mode: 'webtransport',
    },
  };
}
