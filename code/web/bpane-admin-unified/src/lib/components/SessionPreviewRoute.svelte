<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { LogOut, RefreshCw, Unplug } from '@lucide/svelte';
  import { AuthConfigClient, type AuthConfig } from '$lib/auth/auth-config';
  import { BrowserTokenStore } from '$lib/auth/browser-token-store';
  import { OidcAuthClient, OidcAuthClientFactory } from '$lib/auth/oidc-auth-client';
  import type { AuthSnapshot } from '$lib/auth/oidc-types';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { SessionPreviewConnector } from '$lib/session-preview/session-preview-connector';
  import type { LiveBrowserSessionConnection } from '$lib/session-preview/browser-session-types';
  import { SessionCatalogClient } from '$lib/sessions/session-client';
  import type { SessionResource } from '$lib/sessions/session-types';
  import AdminMessage from './AdminMessage.svelte';
  import SessionPreviewMetricsDrawer from './SessionPreviewMetricsDrawer.svelte';

  type SessionPreviewState =
    | { readonly status: 'loading'; readonly label: string }
    | { readonly status: 'connecting'; readonly session: SessionResource }
    | { readonly status: 'connected'; readonly session: SessionResource; readonly gatewayUrl: string }
    | { readonly status: 'disconnected'; readonly session: SessionResource; readonly reason: string }
    | { readonly status: 'error'; readonly message: string };

  type SessionPreviewConnectorLike = {
    readonly connect: (
      session: SessionResource,
      container: HTMLElement,
    ) => Promise<LiveBrowserSessionConnection>;
  };

  type SessionPreviewRouteProps = {
    readonly authContext?: UnifiedAdminContext;
    readonly connectorFactory?: (sessionClient: SessionCatalogClient) => SessionPreviewConnectorLike;
  };

  let {
    authContext: providedAuthContext,
    connectorFactory = (sessionClient) => new SessionPreviewConnector({ sessionClient }),
  }: SessionPreviewRouteProps = $props();

  let authClient = $state<OidcAuthClient | null>(null);
  let authConfig = $state<AuthConfig | null>(null);
  let auth = $state<AuthSnapshot | null>(null);
  let authRedirecting = $state(false);
  let previewAuthContext = $state<UnifiedAdminContext | null>(null);
  let previewState = $state<SessionPreviewState>({ status: 'loading', label: 'Loading preview...' });
  let previewContainer = $state<HTMLDivElement | null>(null);
  let activeConnection = $state<LiveBrowserSessionConnection | null>(null);

  onMount(() => {
    if (providedAuthContext) {
      auth = providedAuthContext.auth;
      previewAuthContext = providedAuthContext;
      void loadAndConnect();
      return;
    }
    void initializeAuth();
  });

  onDestroy(() => {
    disconnectActiveConnection('preview window closed');
  });

  async function initializeAuth(): Promise<void> {
    previewState = { status: 'loading', label: 'Resolving authentication...' };
    try {
      const config = await new AuthConfigClient({ baseUrl: window.location.origin }).load();
      authConfig = config;
      if (!config) {
        previewState = { status: 'error', message: 'OIDC authentication is not configured for this admin preview.' };
        return;
      }

      authClient = OidcAuthClientFactory.create({
        config,
        tokenStore: new BrowserTokenStore(window.sessionStorage),
      });
      await authClient.initialize();
      if (config.mode === 'oidc') {
        await completeLoginRedirect();
      }
      auth = authClient.getSnapshot();
      if (!auth.authenticated && config.mode === 'oidc') {
        await login();
        return;
      }
      previewAuthContext = {
        auth,
        authConfig,
        accessTokenProvider: requireAccessToken,
        onAuthenticationFailure: handleAuthenticationIssue,
        login,
        logout,
      };
      await loadAndConnect();
    } catch (error) {
      previewState = {
        status: 'error',
        message: error instanceof Error ? error.message : 'Unexpected browser preview authentication error.',
      };
    }
  }

  async function completeLoginRedirect(): Promise<void> {
    const currentUrl = new URL(window.location.href);
    const completion = await authClient?.completeLoginIfNeeded(currentUrl);
    if (completion?.completed) {
      window.history.replaceState({}, document.title, completion.cleanUrl);
    }
  }

  async function login(): Promise<void> {
    if (!authClient) {
      return;
    }
    try {
      authRedirecting = true;
      window.location.href = await authClient.buildLoginUrl(new URL(window.location.href));
    } catch (error) {
      authRedirecting = false;
      previewState = {
        status: 'error',
        message: error instanceof Error ? error.message : 'Preview sign-in redirect failed.',
      };
    }
  }

  async function logout(): Promise<void> {
    if (!authClient) {
      return;
    }
    const logoutUrl = await authClient.buildLogoutUrl(new URL(window.location.href));
    auth = authClient.getSnapshot();
    if (logoutUrl) {
      window.location.href = logoutUrl;
    }
  }

  async function requireAccessToken(): Promise<string> {
    const token = await authClient?.getValidAccessToken();
    if (!token) {
      handleAuthenticationIssue();
      throw new Error('No active admin access token');
    }
    auth = authClient.getSnapshot();
    return token;
  }

  function handleAuthenticationIssue(): void {
    if (authRedirecting) {
      return;
    }
    authRedirecting = true;
    authClient?.clear();
    auth = authClient?.getSnapshot() ?? null;
    previewState = {
      status: 'error',
      message: 'Your admin session expired. Redirecting to sign in...',
    };
    void login();
  }

  function client(): SessionCatalogClient {
    if (!previewAuthContext) {
      throw new Error('Preview authentication is not ready.');
    }
    return new SessionCatalogClient({
      baseUrl: window.location.origin,
      accessTokenProvider: previewAuthContext.accessTokenProvider,
      onAuthenticationFailure: previewAuthContext.onAuthenticationFailure,
    });
  }

  async function loadAndConnect(): Promise<void> {
    const sessionId = currentSessionId();
    if (!sessionId) {
      previewState = { status: 'error', message: 'Session id is missing from the current preview route.' };
      return;
    }
    if (!previewContainer) {
      previewState = { status: 'error', message: 'Browser preview container is not available.' };
      return;
    }

    disconnectActiveConnection('preview reconnect');
    previewState = { status: 'loading', label: 'Loading session...' };
    try {
      const sessionClient = client();
      const session = await sessionClient.getSession(sessionId);
      previewState = { status: 'connecting', session };
      activeConnection = await connectorFactory(sessionClient).connect(session, previewContainer);
      previewState = {
        status: 'connected',
        session,
        gatewayUrl: activeConnection.gatewayUrl,
      };
    } catch (error) {
      previewState = {
        status: 'error',
        message: error instanceof Error ? error.message : 'Browser preview connection failed.',
      };
    }
  }

  function disconnect(): void {
    const session = previewSession();
    disconnectActiveConnection('manual disconnect');
    if (session) {
      previewState = { status: 'disconnected', session, reason: 'Disconnected.' };
    }
  }

  function disconnectActiveConnection(_reason: string): void {
    activeConnection?.handle.disconnect();
    activeConnection = null;
  }

  function previewSession(): SessionResource | null {
    if (
      previewState.status === 'connecting'
      || previewState.status === 'connected'
      || previewState.status === 'disconnected'
    ) {
      return previewState.session;
    }
    return null;
  }

  function currentSessionId(): string | null {
    const match = window.location.pathname.match(/\/sessions\/([^/]+)\/preview\/?$/);
    return match?.[1] ? decodeURIComponent(match[1]) : null;
  }

  const statusLabel = $derived(statusText(previewState));

  function statusText(state: SessionPreviewState): string {
    if (state.status === 'loading') {
      return state.label;
    }
    if (state.status === 'connecting') {
      return 'Connecting...';
    }
    if (state.status === 'connected') {
      return 'Connected';
    }
    if (state.status === 'disconnected') {
      return state.reason;
    }
    return 'Connection failed';
  }
</script>

<svelte:head>
  <title>BrowserPane Session Preview</title>
</svelte:head>

<main class="flex h-screen min-h-0 flex-col bg-slate-950 text-white" data-testid="session-preview-route">
  <header class="flex min-h-14 shrink-0 items-center justify-between gap-3 border-b border-white/10 bg-slate-900 px-4">
    <div class="min-w-0">
      <p class="m-0 text-xs font-semibold uppercase text-slate-400">BrowserPane session preview</p>
      <h1 class="m-0 truncate font-mono text-sm font-semibold text-white" data-testid="session-preview-title">
        {previewSession()?.id ?? currentSessionId() ?? 'Session'}
      </h1>
    </div>
    <div class="flex shrink-0 items-center gap-2">
      <span
        class="inline-flex h-8 items-center rounded-md border border-white/10 bg-white/5 px-3 text-xs font-semibold text-slate-200"
        data-testid="session-preview-status"
      >
        {statusLabel}
      </span>
      <button
        class="inline-flex h-8 items-center gap-2 rounded-md border border-white/10 bg-white/5 px-3 text-xs font-semibold text-white hover:bg-white/10 disabled:cursor-not-allowed disabled:opacity-50"
        type="button"
        onclick={() => void loadAndConnect()}
        disabled={previewState.status === 'loading' || previewState.status === 'connecting'}
        data-testid="session-preview-reconnect"
      >
        <RefreshCw size={14} strokeWidth={1.8} />
        <span>Reconnect</span>
      </button>
      <button
        class="inline-flex h-8 items-center gap-2 rounded-md border border-white/10 bg-white/5 px-3 text-xs font-semibold text-white hover:bg-white/10 disabled:cursor-not-allowed disabled:opacity-50"
        type="button"
        onclick={disconnect}
        disabled={previewState.status !== 'connected'}
        data-testid="session-preview-disconnect"
      >
        <Unplug size={14} strokeWidth={1.8} />
        <span>Disconnect</span>
      </button>
      {#if authClient}
        <button
          class="inline-flex h-8 items-center gap-2 rounded-md border border-white/10 bg-white/5 px-3 text-xs font-semibold text-white hover:bg-white/10"
          type="button"
          onclick={() => void logout()}
          data-testid="session-preview-logout"
        >
          <LogOut size={14} strokeWidth={1.8} />
          <span>Sign out</span>
        </button>
      {/if}
    </div>
  </header>

  <section class="relative min-h-0 flex-1 bg-black">
    <div
      class="absolute inset-0 min-h-0 min-w-0 overflow-hidden"
      bind:this={previewContainer}
      data-testid="session-preview-viewport"
    ></div>
    <SessionPreviewMetricsDrawer connection={activeConnection} />

    {#if previewState.status === 'loading' || previewState.status === 'connecting'}
      <div class="absolute inset-0 grid place-items-center bg-black/70 p-6">
        <AdminMessage tone="loading" title={statusLabel} testId="session-preview-loading" />
      </div>
    {:else if previewState.status === 'error'}
      <div class="absolute inset-0 grid place-items-center bg-black/80 p-6">
        <div class="w-full max-w-[720px]" data-testid="session-preview-error">
          <AdminMessage tone="error" title="Browser preview unavailable" message={previewState.message} />
        </div>
      </div>
    {:else if previewState.status === 'disconnected'}
      <div class="absolute inset-0 grid place-items-center bg-black/80 p-6">
        <AdminMessage
          tone="info"
          title="Preview disconnected"
          message="Reconnect to attach this popup to the selected browser session again."
          testId="session-preview-disconnected"
        />
      </div>
    {/if}
  </section>
</main>
