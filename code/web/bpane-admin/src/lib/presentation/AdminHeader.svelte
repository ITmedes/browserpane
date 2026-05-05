<script lang="ts">
  import type { AuthSnapshot } from '../auth/oidc-types';

  type AdminHeaderProps = {
    readonly auth: AuthSnapshot | null;
    readonly authError: string | null;
    readonly loading: boolean;
    readonly onLogin: () => void;
    readonly onLogout: () => void;
  };

  let { auth, authError, loading, onLogin, onLogout }: AdminHeaderProps = $props();
  const authLabel = $derived(auth?.authenticated ? auth.username : loading ? 'loading auth' : 'signed out');
</script>

<header class="fixed inset-x-0 top-0 z-50 border-b border-admin-ink/12 bg-admin-cream/88 shadow-[0_10px_30px_rgb(24_32_24_/_9%)] backdrop-blur-xl">
  <div class="mx-auto flex h-14 w-[calc(100vw-32px)] max-w-[1680px] items-center justify-between gap-3">
    <nav class="min-w-0" aria-label="Breadcrumb">
      <ol class="flex min-w-0 items-center gap-2 text-sm font-extrabold text-admin-ink/62">
        <li><a class="text-admin-leaf no-underline" href="/">BrowserPane</a></li>
        <li aria-hidden="true">/</li>
        <li><span class="text-admin-ink/78">Admin</span></li>
        <li class="max-[620px]:hidden" aria-hidden="true">/</li>
        <li class="max-[620px]:hidden">
          <span class="block max-w-[220px] overflow-hidden text-ellipsis whitespace-nowrap text-admin-ink">Workspace</span>
        </li>
      </ol>
    </nav>

    <div class="flex min-w-0 items-center justify-end gap-2">
      {#if authError}
        <span class="hidden max-w-[300px] overflow-hidden text-ellipsis whitespace-nowrap rounded-full bg-admin-danger/10 px-3 py-1 text-xs font-bold text-admin-danger md:block">
          {authError}
        </span>
      {/if}
      <a class="admin-header-link max-[760px]:hidden" href="/auth-config.json">Auth</a>
      <a class="admin-header-link max-[900px]:hidden" href="/cert-fingerprint">Cert</a>
      <span class="max-w-[180px] overflow-hidden text-ellipsis whitespace-nowrap rounded-full bg-admin-leaf/10 px-3 py-1 text-xs font-extrabold text-admin-leaf">
        {authLabel}
      </span>
      {#if auth?.authenticated}
        <button class="admin-header-button" type="button" data-testid="admin-logout" onclick={onLogout}>Sign out</button>
      {:else if auth?.configured}
        <button class="admin-header-button" type="button" data-testid="admin-login" onclick={onLogin}>Sign in</button>
      {/if}
    </div>
  </div>
</header>
