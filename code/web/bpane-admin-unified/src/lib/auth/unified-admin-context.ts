import type { AuthConfig } from './auth-config';
import type { AuthSnapshot } from './oidc-types';

export type UnifiedAdminContext = {
  readonly auth: AuthSnapshot;
  readonly authConfig: AuthConfig | null;
  readonly accessTokenProvider: () => Promise<string>;
  readonly onAuthenticationFailure: () => void;
  readonly login: () => Promise<void>;
  readonly logout: () => Promise<void>;
};
