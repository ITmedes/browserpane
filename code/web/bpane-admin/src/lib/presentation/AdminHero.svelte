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
    <p class="admin-eyebrow mb-[18px]">Reference admin foundation</p>
    <h1 class="m-0 max-w-[860px] font-display text-[clamp(3rem,8vw,6.8rem)] leading-[0.9] font-bold tracking-[-0.07em] text-admin-ink">
      BrowserPane operations without the test-page drift.
    </h1>
    <p class="mt-7 mb-0 max-w-[680px] text-[clamp(1.05rem,2vw,1.3rem)] leading-[1.55] text-admin-ink/78">
      This console authenticates with the local OIDC setup and talks to the
      frozen owner-scoped control API before adding deeper operations surfaces.
    </p>
  </div>

  <aside
    class="relative z-10 self-end rounded-[28px] border border-admin-ink/14 bg-admin-panel/78 p-6 shadow-[0_18px_48px_rgb(24_32_24_/_12%)]"
    aria-label="Authentication state"
  >
    <p class="admin-eyebrow mb-[18px]">Operator access</p>
    {#if loading}
      <p class="m-0 leading-normal text-admin-ink/78">Loading auth metadata...</p>
    {:else if auth?.authenticated}
      <p class="m-0 leading-normal text-admin-ink/78">Signed in as <strong>{auth.username}</strong></p>
      <button class="admin-button-primary mt-[18px]" type="button" data-testid="admin-logout" onclick={onLogout}>Sign out</button>
    {:else if auth?.configured}
      <p class="m-0 leading-normal text-admin-ink/78">Sign in with the local BrowserPane realm.</p>
      <button class="admin-button-primary mt-[18px]" type="button" data-testid="admin-login" onclick={onLogin}>Sign in</button>
    {:else}
      <p class="m-0 leading-normal text-admin-ink/78">OIDC is not configured for this deployment.</p>
    {/if}

    {#if authError}
      <p class="admin-error text-[0.92rem]">{authError}</p>
    {/if}

    <div class="mt-[22px] flex flex-wrap gap-2.5">
      <a class="admin-button-ghost" href="/auth-config.json">Auth config</a>
      <a class="admin-button-ghost" href="/cert-fingerprint">Certificate fingerprint</a>
    </div>
  </aside>
</section>
