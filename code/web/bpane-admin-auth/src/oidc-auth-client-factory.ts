import type { AuthConfig } from './auth-config';
import { BrowserTokenStore } from './browser-token-store';
import { OidcAuthClient } from './oidc-auth-client';
import { OidcEndpointClient } from './oidc-endpoint-client';
import { OidcLoginTransaction } from './oidc-login-transaction';

export type OidcAuthClientOptions = {
  readonly config: AuthConfig;
  readonly tokenStore: BrowserTokenStore;
  readonly fetchImpl?: typeof fetch;
  readonly nowMs?: () => number;
};

export class OidcAuthClientFactory {
  static create(options: OidcAuthClientOptions): OidcAuthClient {
    const fetchImpl = options.fetchImpl ?? fetch;
    const nowMs = options.nowMs ?? Date.now;
    const endpoints = new OidcEndpointClient(options.config, fetchImpl, nowMs);
    const loginTransaction = new OidcLoginTransaction(options.tokenStore, nowMs);
    return new OidcAuthClient({
      config: options.config,
      tokenStore: options.tokenStore,
      endpoints,
      loginTransaction,
      nowMs,
    });
  }
}
