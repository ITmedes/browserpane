<script lang="ts">
  import { onMount } from 'svelte';
  import type { Snippet } from 'svelte';
  import { AuthConfigClient, type AuthConfig } from '$lib/auth/auth-config';
  import { BrowserTokenStore } from '$lib/auth/browser-token-store';
  import { OidcAuthClient } from '$lib/auth/oidc-auth-client';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import type { AuthSnapshot } from '$lib/auth/oidc-types';
  import ShellAccountButton from '$lib/components/ShellAccountButton.svelte';
  import ShellLogo from '$lib/components/ShellLogo.svelte';
  import ShellNavigation from '$lib/components/ShellNavigation.svelte';
  import ShellNotificationsButton from '$lib/components/ShellNotificationsButton.svelte';
  import ShellSearch from '$lib/components/ShellSearch.svelte';

  type UnifiedAdminShellProps = {
    readonly activeId?: string;
    readonly title?: string;
    readonly children?: Snippet<[UnifiedAdminContext]>;
  };

  let {
    activeId = 'dashboard',
    title = 'BrowserPane Admin Redesign',
    children,
  }: UnifiedAdminShellProps = $props();

  let authClient = $state<OidcAuthClient | null>(null);
  let authConfig = $state<AuthConfig | null>(null);
  let auth = $state<AuthSnapshot | null>(null);
  let authLoading = $state(true);
  let authError = $state<string | null>(null);
  let authRedirecting = $state(false);

  const routeContext = $derived(auth?.authenticated && authClient
    ? {
        auth,
        authConfig,
        accessTokenProvider: requireAccessToken,
        onAuthenticationFailure: handleAuthenticationIssue,
        login,
        logout,
      }
    : null);

  onMount(() => {
    void initializeAuth();
  });

  async function initializeAuth(): Promise<void> {
    try {
      const config = await new AuthConfigClient({ baseUrl: window.location.origin }).load();
      authConfig = config;
      if (!config) {
        auth = {
          configured: false,
          authenticated: false,
          username: '--',
          accessToken: null,
          claims: null,
        };
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
    } catch (error) {
      authError = errorMessage(error);
    } finally {
      authLoading = false;
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
      authError = errorMessage(error);
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
    authError = 'Your admin session expired. Redirecting to sign in...';
    void login();
  }

  function errorMessage(error: unknown): string {
    return error instanceof Error ? error.message : 'Unexpected admin authentication error';
  }
</script>

<svelte:head>
  <title>{title}</title>
</svelte:head>

<main class="min-h-screen bg-admin-bg text-admin-ink" data-testid="admin-new-shell">
  <section class="flex min-h-screen flex-col overflow-hidden bg-admin-paper">
    <header
      class="flex min-h-16 shrink-0 items-center gap-3 border-b border-admin-border bg-admin-panel px-4 sm:px-5"
      data-testid="admin-new-header">
      <ShellLogo />
      <ShellSearch />
      <ShellNotificationsButton />
      <ShellAccountButton
        label={auth?.authenticated ? `Account: ${auth.username}. Click to sign out.` : 'Account'}
        onClick={() => {
          if (auth?.authenticated) {
            void logout();
          } else {
            void login();
          }
        }}
      />
    </header>

    <div class="flex min-h-0 flex-1">
      <ShellNavigation {activeId} />
      <section class="min-w-0 flex-1 overflow-y-auto bg-admin-bg" aria-label="Workspace">
        {#if routeContext && children}
          {@render children(routeContext)}
        {:else if !routeContext}
          <div class="mx-auto flex min-h-full w-full max-w-[960px] items-center px-4 py-6 sm:px-6">
            <section
              class="w-full rounded-md border border-admin-border bg-admin-panel p-5 shadow-sm"
              data-testid="admin-new-auth-gate"
            >
              {#if authLoading}
                <p class="m-0 text-sm text-admin-muted">Loading authentication metadata...</p>
              {:else if authError && !auth?.authenticated}
                <div class="rounded-md border border-red-200 bg-red-50 p-3 text-sm text-red-900" role="alert">
                  {authError}
                </div>
              {/if}

              {#if !authLoading && !auth?.authenticated}
                <p class="m-0 text-xs font-semibold uppercase tracking-[0.14em] text-admin-muted">Operator access</p>
                <h1 class="m-0 mt-2 text-xl font-semibold text-admin-ink">Sign in to BrowserPane</h1>
                {#if auth?.configured}
                  <p class="m-0 mt-2 text-sm text-admin-muted">Use the configured Keycloak realm to access this admin route.</p>
                  <button
                    class="mt-4 inline-flex h-10 items-center rounded-md bg-admin-accent px-4 text-sm font-semibold text-white hover:bg-indigo-600"
                    type="button"
                    onclick={() => void login()}
                    disabled={authRedirecting}
                    data-testid="admin-new-login-button"
                  >
                    Sign in
                  </button>
                {:else if !authError}
                  <p class="m-0 mt-2 text-sm text-admin-muted">OIDC is not configured for this deployment.</p>
                {/if}
              {/if}
            </section>
          </div>
        {/if}
      </section>
    </div>
  </section>
</main>
