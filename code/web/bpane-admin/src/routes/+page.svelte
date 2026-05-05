<script lang="ts">
  import { onMount } from 'svelte';
  import { ControlClient } from '$lib/api/control-client';
  import type { SessionResource } from '$lib/api/control-types';
  import { AuthConfigClient } from '$lib/auth/auth-config';
  import { BrowserTokenStore } from '$lib/auth/browser-token-store';
  import { OidcAuthClient } from '$lib/auth/oidc-auth-client';
  import type { AuthSnapshot } from '$lib/auth/oidc-types';
  import AdminHero from '$lib/presentation/AdminHero.svelte';
  import SessionListPanel from '$lib/presentation/SessionListPanel.svelte';

  let authClient: OidcAuthClient | null = null;
  let controlClient: ControlClient | null = null;
  let auth: AuthSnapshot | null = null;
  let authLoading = true;
  let authError: string | null = null;
  let sessions: readonly SessionResource[] = [];
  let sessionsLoading = false;
  let sessionsError: string | null = null;

  onMount(() => {
    void initialize();
  });

  async function initialize(): Promise<void> {
    try {
      const config = await new AuthConfigClient({ baseUrl: window.location.origin }).load();
      if (!config) {
        auth = null;
        return;
      }
      authClient = new OidcAuthClient({
        config,
        tokenStore: new BrowserTokenStore(window.sessionStorage),
      });
      if (config.mode === 'oidc') {
        await completeLoginRedirect();
      }
      auth = authClient.getSnapshot();
      if (auth.authenticated) {
        bindControlClient();
        await loadSessions();
      }
    } catch (error) {
      authError = errorMessage(error);
    } finally {
      authLoading = false;
    }
  }

  async function login(): Promise<void> {
    if (!authClient) {
      return;
    }
    window.location.href = await authClient.buildLoginUrl(new URL(window.location.href));
  }

  async function logout(): Promise<void> {
    if (!authClient) {
      return;
    }
    const logoutUrl = await authClient.buildLogoutUrl(new URL(window.location.href));
    auth = authClient.getSnapshot();
    sessions = [];
    if (logoutUrl) {
      window.location.href = logoutUrl;
    }
  }

  async function loadSessions(): Promise<void> {
    if (!controlClient) {
      return;
    }
    sessionsLoading = true;
    sessionsError = null;
    try {
      sessions = (await controlClient.listSessions()).sessions;
    } catch (error) {
      sessionsError = errorMessage(error);
    } finally {
      sessionsLoading = false;
    }
  }

  async function createSession(): Promise<void> {
    if (!controlClient) {
      return;
    }
    sessionsLoading = true;
    sessionsError = null;
    try {
      await controlClient.createSession();
      sessions = (await controlClient.listSessions()).sessions;
    } catch (error) {
      sessionsError = errorMessage(error);
    } finally {
      sessionsLoading = false;
    }
  }

  async function completeLoginRedirect(): Promise<void> {
    const currentUrl = new URL(window.location.href);
    const completion = await authClient?.completeLoginIfNeeded(currentUrl);
    if (completion?.completed) {
      window.history.replaceState({}, document.title, completion.cleanUrl);
    }
  }

  function bindControlClient(): void {
    if (!authClient) {
      return;
    }
    controlClient = new ControlClient({
      baseUrl: window.location.origin,
      accessTokenProvider: async () => {
        const token = await authClient?.getValidAccessToken();
        if (!token) {
          throw new Error('No active admin access token');
        }
        return token;
      },
    });
  }

  function errorMessage(error: unknown): string {
    return error instanceof Error ? error.message : 'Unexpected admin console error';
  }
</script>

<svelte:head>
  <title>BrowserPane Admin</title>
</svelte:head>

<main class="admin-shell">
  <AdminHero
    {auth}
    {authError}
    loading={authLoading}
    onLogin={() => void login()}
    onLogout={() => void logout()}
  />

  <SessionListPanel
    {sessions}
    authenticated={Boolean(auth?.authenticated)}
    loading={sessionsLoading}
    error={sessionsError}
    onRefresh={() => void loadSessions()}
    onCreateSession={() => void createSession()}
  />
</main>

<style>
  .admin-shell {
    width: min(1120px, calc(100vw - 32px));
    margin: 0 auto;
    padding: 72px 0;
  }

  @media (max-width: 760px) {
    .admin-shell {
      padding: 32px 0;
    }
  }
</style>
