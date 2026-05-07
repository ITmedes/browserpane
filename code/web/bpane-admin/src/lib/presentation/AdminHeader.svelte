<script lang="ts">
  import { base } from '$app/paths';
  import { LogIn, LogOut, PanelRightClose, PanelRightOpen, UserRound } from 'lucide-svelte';
  import type { AuthSnapshot } from '../auth/oidc-types';

  type AdminHeaderProps = {
    readonly auth: AuthSnapshot | null;
    readonly authError: string | null;
    readonly loading: boolean;
    readonly adminOpen: boolean;
    readonly showAdminToggle: boolean;
    readonly onLogin: () => void;
    readonly onLogout: () => void;
    readonly onAdminToggle: () => void;
  };

  let {
    auth,
    authError,
    loading,
    adminOpen,
    showAdminToggle,
    onLogin,
    onLogout,
    onAdminToggle,
  }: AdminHeaderProps = $props();
  const authLabel = $derived(auth?.authenticated ? auth.username : loading ? 'loading auth' : 'signed out');
  const logoUrl = `${base}/browserpane-logo.png`;
</script>

<header class="fixed inset-x-0 top-0 z-50 border-b border-[#90a6cc]/20 bg-[#0d1522]/90 shadow-[0_18px_44px_rgb(0_0_0_/_28%)] backdrop-blur-xl">
  <div class="mx-auto flex h-14 w-[calc(100vw-32px)] max-w-[1680px] items-center justify-between gap-3">
    <div class="flex min-w-0 items-center gap-3">
      <a class="block shrink-0" href="/" aria-label="BrowserPane home">
        <img class="h-10 w-36 object-cover object-center sm:w-44" src={logoUrl} alt="BrowserPane" />
      </a>
    </div>

    <div class="flex min-w-0 items-center justify-end gap-2">
      {#if authError}
        <span class="hidden max-w-[300px] overflow-hidden text-ellipsis whitespace-nowrap rounded-full bg-admin-danger/10 px-3 py-1 text-xs font-bold text-admin-danger md:block">
          {authError}
        </span>
      {/if}
      {#if showAdminToggle}
        <button
          class="inline-flex items-center gap-1.5 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 py-1.5 text-xs font-bold text-admin-ink"
          type="button"
          data-testid={adminOpen ? 'admin-overlay-close-header' : 'admin-overlay-open'}
          onclick={onAdminToggle}
        >
          {#if adminOpen}
            <PanelRightClose size={15} aria-hidden="true" />
            <span class="max-[560px]:hidden">Hide admin</span>
          {:else}
            <PanelRightOpen size={15} aria-hidden="true" />
            <span class="max-[560px]:hidden">Admin</span>
          {/if}
        </button>
      {/if}
      <span class="inline-flex max-w-[180px] items-center gap-1.5 overflow-hidden text-ellipsis whitespace-nowrap rounded-xl border border-admin-leaf/25 bg-admin-leaf/10 px-3 py-1 text-xs font-bold text-admin-leaf">
        <UserRound size={14} aria-hidden="true" />
        {authLabel}
      </span>
      {#if auth?.authenticated}
        <button class="inline-flex items-center gap-1.5 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 py-1.5 text-xs font-bold text-admin-ink" type="button" data-testid="admin-logout" onclick={onLogout}>
          <LogOut size={14} aria-hidden="true" />
          <span class="max-[640px]:hidden">Sign out</span>
        </button>
      {:else if auth?.configured}
        <button class="inline-flex items-center gap-1.5 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 py-1.5 text-xs font-bold text-admin-ink" type="button" data-testid="admin-login" onclick={onLogin}>
          <LogIn size={14} aria-hidden="true" />
          <span>Sign in</span>
        </button>
      {/if}
    </div>
  </div>
</header>
