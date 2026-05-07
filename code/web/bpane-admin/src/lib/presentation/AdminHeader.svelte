<script lang="ts">
  import { base } from '$app/paths';
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
  const logoUrl = `${base}/browserpane-logo.png`;
</script>

<header class="fixed inset-x-0 top-0 z-50 border-b border-[#90a6cc]/20 bg-[#0d1522]/90 shadow-[0_18px_44px_rgb(0_0_0_/_28%)] backdrop-blur-xl">
  <div class="mx-auto flex h-14 w-[calc(100vw-32px)] max-w-[1680px] items-center justify-between gap-3">
    <div class="flex min-w-0 items-center gap-3">
      <a class="block shrink-0 overflow-hidden rounded-lg border border-[#90a6cc]/20 bg-[#111e32]/80" href="/" aria-label="BrowserPane home">
        <img class="h-10 w-36 object-cover object-center sm:w-44" src={logoUrl} alt="BrowserPane" />
      </a>
      <nav class="min-w-0" aria-label="Breadcrumb">
        <ol class="flex min-w-0 items-center gap-2 text-sm font-bold text-[#9fb1cf]">
          <li class="max-[520px]:hidden"><span>Admin</span></li>
          <li class="max-[620px]:hidden" aria-hidden="true">/</li>
          <li class="max-[620px]:hidden">
            <span class="block max-w-[220px] overflow-hidden text-ellipsis whitespace-nowrap text-admin-ink">Workspace</span>
          </li>
        </ol>
      </nav>
    </div>

    <div class="flex min-w-0 items-center justify-end gap-2">
      {#if authError}
        <span class="hidden max-w-[300px] overflow-hidden text-ellipsis whitespace-nowrap rounded-full bg-admin-danger/10 px-3 py-1 text-xs font-bold text-admin-danger md:block">
          {authError}
        </span>
      {/if}
      <span class="max-w-[180px] overflow-hidden text-ellipsis whitespace-nowrap rounded-xl border border-admin-leaf/25 bg-admin-leaf/10 px-3 py-1 text-xs font-bold text-admin-leaf">
        {authLabel}
      </span>
      {#if auth?.authenticated}
        <button class="rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 py-1.5 text-xs font-bold text-admin-ink" type="button" data-testid="admin-logout" onclick={onLogout}>Sign out</button>
      {:else if auth?.configured}
        <button class="rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 py-1.5 text-xs font-bold text-admin-ink" type="button" data-testid="admin-login" onclick={onLogin}>Sign in</button>
      {/if}
    </div>
  </div>
</header>
