<script lang="ts">
  import type { AuthSnapshot } from '../auth/oidc-types';

  type AdminHeroProps = {
    readonly auth: AuthSnapshot | null;
    readonly authError: string | null;
    readonly loading: boolean;
    readonly onLogin: () => void;
    readonly onLogout: () => void;
  };

  let { auth, authError, loading, onLogin, onLogout }: AdminHeroProps = $props();
</script>

<section class="admin-hero">
  <div>
    <p class="admin-eyebrow">Operator console</p>
    <h1 class="m-0 max-w-[860px] font-display text-[clamp(2rem,4vw,4.2rem)] leading-[0.92] font-bold tracking-[-0.055em] text-admin-ink">
      BrowserPane admin, browser-first.
    </h1>
    <p class="mt-3 mb-0 max-w-[760px] text-sm leading-normal text-admin-ink/72 md:text-base">
      The live browser stays dominant. Operational surfaces from the dev harness
      move into collapsible, view-model-backed panels.
    </p>
  </div>

  <aside
    class="relative z-10 self-center rounded-[24px] border border-admin-ink/14 bg-admin-panel/78 p-4 shadow-[0_12px_36px_rgb(24_32_24_/_10%)]"
    aria-label="Authentication state"
  >
    <p class="admin-eyebrow mb-2">Operator access</p>
    {#if loading}
      <p class="m-0 leading-normal text-admin-ink/78">Loading auth metadata...</p>
    {:else if auth?.authenticated}
      <p class="m-0 leading-normal text-admin-ink/78">Signed in as <strong>{auth.username}</strong></p>
      <button class="admin-button-primary mt-3" type="button" data-testid="admin-logout" onclick={onLogout}>Sign out</button>
    {:else if auth?.configured}
      <p class="m-0 leading-normal text-admin-ink/78">Sign in with the local BrowserPane realm.</p>
      <button class="admin-button-primary mt-3" type="button" data-testid="admin-login" onclick={onLogin}>Sign in</button>
    {:else}
      <p class="m-0 leading-normal text-admin-ink/78">OIDC is not configured for this deployment.</p>
    {/if}

    {#if authError}
      <p class="admin-error text-[0.92rem]">{authError}</p>
    {/if}

    <div class="mt-3 flex flex-wrap gap-2.5">
      <a class="admin-button-ghost" href="/auth-config.json">Auth config</a>
      <a class="admin-button-ghost" href="/cert-fingerprint">Certificate fingerprint</a>
    </div>
  </aside>
</section>
