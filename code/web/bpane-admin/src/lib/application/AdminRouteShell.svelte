<script lang="ts">
  import { onMount, type Snippet } from 'svelte';
  import { AdminEventClient } from '../api/admin-event-client';
  import { ControlClient } from '../api/control-client';
  import { WorkflowClient } from '../api/workflow-client';
  import { AuthConfigClient, type AuthConfig } from '../auth/auth-config';
  import { BrowserTokenStore } from '../auth/browser-token-store';
  import { OidcAuthClient } from '../auth/oidc-auth-client';
  import type { AuthSnapshot } from '../auth/oidc-types';
  import AdminHeader from '../presentation/AdminHeader.svelte';
  import type { AdminRouteContext } from './admin-route-context';

  type AdminRouteShellProps = {
    readonly title?: string;
    readonly contentClass?: string;
    readonly showAdminToggle?: boolean;
    readonly children: Snippet<[AdminRouteContext]>;
  };

  let {
    title = 'BrowserPane Admin',
    contentClass = 'mx-auto min-h-screen w-[calc(100vw-20px)] max-w-[1680px] pt-[78px] pb-3 sm:w-[calc(100vw-32px)] sm:pt-20 sm:pb-4',
    showAdminToggle = false,
    children,
  }: AdminRouteShellProps = $props();

  let authClient = $state<OidcAuthClient | null>(null);
  let controlClient = $state<ControlClient | null>(null);
  let adminEventClient = $state<AdminEventClient | null>(null);
  let workflowClient = $state<WorkflowClient | null>(null);
  let authConfig = $state<AuthConfig | null>(null);
  let auth = $state<AuthSnapshot | null>(null);
  let authLoading = $state(true);
  let authError = $state<string | null>(null);
  let authRedirecting = $state(false);
  let adminOpen = $state(true);

  const routeContext = $derived(auth?.authenticated && controlClient && adminEventClient && workflowClient
    ? {
        auth,
        authConfig,
        controlClient,
        adminEventClient,
        workflowClient,
        adminOpen,
        setAdminOpen,
      }
    : null);

  onMount(() => {
    void initialize();
  });

  async function initialize(): Promise<void> {
    try {
      const config = await new AuthConfigClient({ baseUrl: window.location.origin }).load();
      authConfig = config;
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
        bindAuthenticatedClients();
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
    try {
      window.location.href = await authClient.buildLoginUrl(new URL(window.location.href));
    } catch (error) {
      authRedirecting = false;
      authError = errorMessage(error);
    }
  }

  async function logout(): Promise<void> {
    if (!authClient) {
      return;
    }
    const logoutUrl = await authClient.buildLogoutUrl(new URL(window.location.href));
    auth = authClient.getSnapshot();
    clearAuthenticatedClients();
    if (logoutUrl) {
      window.location.href = logoutUrl;
    }
  }

  async function completeLoginRedirect(): Promise<void> {
    const currentUrl = new URL(window.location.href);
    const completion = await authClient?.completeLoginIfNeeded(currentUrl);
    if (completion?.completed) {
      window.history.replaceState({}, document.title, completion.cleanUrl);
    }
  }

  function bindAuthenticatedClients(): void {
    if (!authClient) {
      return;
    }
    controlClient = new ControlClient({
      baseUrl: window.location.origin,
      accessTokenProvider: requireAccessToken,
      onAuthenticationFailure: handleAuthenticationIssue,
    });
    adminEventClient = new AdminEventClient({
      baseUrl: window.location.origin,
      accessTokenProvider: requireAccessToken,
    });
    workflowClient = new WorkflowClient({
      baseUrl: window.location.origin,
      accessTokenProvider: requireAccessToken,
      onAuthenticationFailure: handleAuthenticationIssue,
    });
  }

  async function requireAccessToken(): Promise<string> {
    const token = await authClient?.getValidAccessToken();
    if (!token) {
      handleAuthenticationIssue();
      throw new Error('No active admin access token');
    }
    return token;
  }

  function handleAuthenticationIssue(): void {
    if (authRedirecting) {
      return;
    }
    authRedirecting = true;
    authClient?.clear();
    auth = authClient?.getSnapshot() ?? null;
    clearAuthenticatedClients();
    authError = 'Your admin session expired. Redirecting to sign in...';
    void login();
  }

  function clearAuthenticatedClients(): void {
    controlClient = null;
    adminEventClient = null;
    workflowClient = null;
  }

  function setAdminOpen(open: boolean): void {
    adminOpen = open;
  }

  function errorMessage(error: unknown): string {
    return error instanceof Error ? error.message : 'Unexpected admin console error';
  }
</script>

<svelte:head>
  <title>{title}</title>
</svelte:head>

<AdminHeader
  {auth}
  {authError}
  adminOpen={adminOpen}
  loading={authLoading}
  showAdminToggle={showAdminToggle && Boolean(routeContext)}
  onLogin={() => void login()}
  onLogout={() => void logout()}
  onAdminToggle={() => { adminOpen = !adminOpen; }}
/>

<main class={contentClass}>
  {#if authError && !auth?.authenticated}
    <section class="admin-panel">
      <p class="admin-error mt-0">{authError}</p>
    </section>
  {/if}

  {#if !auth?.authenticated}
    <section class="rounded-2xl border border-[#90a6cc]/20 bg-admin-panel/90 p-5 shadow-[0_24px_64px_rgb(0_0_0_/_34%)]">
      <p class="admin-eyebrow">Operator access</p>
      {#if authLoading}
        <p class="m-0 leading-normal text-admin-ink/78">Loading auth metadata...</p>
      {:else if auth?.configured}
        <p class="m-0 leading-normal text-admin-ink/78">Sign in with the local BrowserPane realm.</p>
        <button class="admin-button-primary mt-3" type="button" onclick={() => void login()}>Sign in</button>
      {:else}
        <p class="m-0 leading-normal text-admin-ink/78">OIDC is not configured for this deployment.</p>
      {/if}
    </section>
  {/if}

  {#if routeContext}
    {@render children(routeContext)}
  {/if}
</main>
