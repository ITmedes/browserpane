<script lang="ts">
  import { onMount } from 'svelte';
  import { ControlClient } from '$lib/api/control-client';
  import { AuthConfigClient, type AuthConfig } from '$lib/auth/auth-config';
  import { BrowserTokenStore } from '$lib/auth/browser-token-store';
  import { OidcAuthClient } from '$lib/auth/oidc-auth-client';
  import type { AuthSnapshot } from '$lib/auth/oidc-types';
  import AdminSessionSurface from '$lib/application/AdminSessionSurface.svelte';
  import AdminHeader from '$lib/presentation/AdminHeader.svelte';

  let authClient = $state<OidcAuthClient | null>(null);
  let controlClient = $state<ControlClient | null>(null);
  let authConfig = $state<AuthConfig | null>(null);
  let auth = $state<AuthSnapshot | null>(null);
  let authLoading = $state(true);
  let authError = $state<string | null>(null);
  let adminOpen = $state(true);

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
        bindControlClient();
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
    controlClient = null;
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

<AdminHeader
  {auth}
  {authError}
  {adminOpen}
  loading={authLoading}
  showAdminToggle={Boolean(auth?.authenticated && controlClient)}
  onLogin={() => void login()}
  onLogout={() => void logout()}
  onAdminToggle={() => { adminOpen = !adminOpen; }}
/>

<main class="mx-auto min-h-screen w-[calc(100vw-20px)] max-w-[1680px] pt-[78px] pb-3 sm:w-[calc(100vw-32px)] sm:pt-20 sm:pb-4">
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

  {#if auth?.authenticated && controlClient}
    <AdminSessionSurface
      {controlClient}
      {adminOpen}
      mcpBridge={authConfig?.mcpBridge ?? null}
      onAdminOpenChange={(open) => { adminOpen = open; }}
    />
  {/if}
</main>
