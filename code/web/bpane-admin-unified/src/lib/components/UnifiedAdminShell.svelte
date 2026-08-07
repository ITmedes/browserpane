<script lang="ts">
  import { replaceState } from '$app/navigation';
  import { onMount } from 'svelte';
  import type { Snippet } from 'svelte';
  import { AuthConfigClient, type AuthConfig } from '$lib/auth/auth-config';
  import { BrowserTokenStore } from '$lib/auth/browser-token-store';
  import { OidcAuthClient, OidcAuthClientFactory } from '$lib/auth/oidc-auth-client';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import type { AuthSnapshot } from '$lib/auth/oidc-types';
  import AdminMessage from '$lib/components/AdminMessage.svelte';
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
      }
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
      replaceState(completion.cleanUrl, {});
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
                <AdminMessage
                  tone="loading"
                  title="Loading authentication metadata"
                  message="The admin shell is resolving the configured sign-in provider."
                  testId="admin-new-auth-loading"
                />
              {:else if authError && !auth?.authenticated}
                <AdminMessage
                  tone="error"
                  title="Authentication required"
                  message={authError}
                  testId="admin-new-auth-error"
                />
              {/if}

              {#if !authLoading && !auth?.authenticated}
                {#if auth?.configured && !authError}
                  <AdminMessage
                    tone="loading"
                    title="Redirecting to sign in"
                    message="Keycloak sign-in is opening for this admin session."
                    testId="admin-new-auth-redirecting"
                  />
                {:else if !authError}
                  <AdminMessage
                    tone="info"
                    title="OIDC is not configured"
                    message="This deployment does not expose an admin sign-in provider."
                    testId="admin-new-auth-unconfigured"
                  />
                {/if}
              {/if}
            </section>
          </div>
        {/if}
      </section>
    </div>
  </section>
</main>
