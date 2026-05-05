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

<section class="hero">
  <div>
    <p class="eyebrow">Reference admin foundation</p>
    <h1>BrowserPane operations without the test-page drift.</h1>
    <p class="lede">
      This console authenticates with the local OIDC setup and talks to the
      frozen owner-scoped control API before adding deeper operations surfaces.
    </p>
  </div>

  <aside class="auth-card" aria-label="Authentication state">
    <p class="card-label">Operator access</p>
    {#if loading}
      <p class="state">Loading auth metadata...</p>
    {:else if auth?.authenticated}
      <p class="state">Signed in as <strong>{auth.username}</strong></p>
      <button type="button" onclick={onLogout}>Sign out</button>
    {:else if auth?.configured}
      <p class="state">Sign in with the local BrowserPane realm.</p>
      <button type="button" onclick={onLogin}>Sign in</button>
    {:else}
      <p class="state">OIDC is not configured for this deployment.</p>
    {/if}

    {#if authError}
      <p class="error">{authError}</p>
    {/if}

    <div class="links">
      <a href="/auth-config.json">Auth config</a>
      <a href="/cert-fingerprint">Certificate fingerprint</a>
    </div>
  </aside>
</section>

<style>
  .hero {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(280px, 360px);
    gap: clamp(24px, 5vw, 56px);
    overflow: hidden;
    padding: clamp(32px, 6vw, 72px);
    border: 1px solid rgba(24, 32, 24, 0.14);
    border-radius: 36px;
    background:
      linear-gradient(120deg, rgba(255, 255, 248, 0.9), rgba(238, 232, 204, 0.72)),
      repeating-linear-gradient(90deg, transparent 0 24px, rgba(24, 32, 24, 0.035) 24px 25px);
    box-shadow: 0 28px 80px rgba(24, 32, 24, 0.14);
  }

  .hero::after {
    position: absolute;
    right: -56px;
    bottom: -80px;
    width: 260px;
    height: 260px;
    border-radius: 48%;
    background: #db6b3b;
    content: "";
    opacity: 0.18;
    transform: rotate(-18deg);
  }

  .eyebrow,
  .card-label {
    margin: 0 0 18px;
    color: #417463;
    font-size: 0.78rem;
    font-weight: 800;
    letter-spacing: 0.16em;
    text-transform: uppercase;
  }

  h1 {
    max-width: 860px;
    margin: 0;
    color: #162119;
    font-family: "Georgia", "Charter", serif;
    font-size: clamp(3rem, 8vw, 6.8rem);
    line-height: 0.9;
    letter-spacing: -0.07em;
  }

  .lede {
    max-width: 680px;
    margin: 28px 0 0;
    color: rgba(24, 32, 24, 0.78);
    font-size: clamp(1.05rem, 2vw, 1.3rem);
    line-height: 1.55;
  }

  .auth-card {
    z-index: 1;
    align-self: end;
    padding: 24px;
    border: 1px solid rgba(24, 32, 24, 0.14);
    border-radius: 28px;
    background: rgba(255, 255, 248, 0.78);
    box-shadow: 0 18px 48px rgba(24, 32, 24, 0.12);
  }

  .state {
    margin: 0;
    color: rgba(24, 32, 24, 0.78);
    line-height: 1.5;
  }

  button,
  .links a {
    display: inline-flex;
    align-items: center;
    min-height: 42px;
    padding: 0 16px;
    border: 1px solid rgba(24, 32, 24, 0.18);
    border-radius: 999px;
    color: #162119;
    background: rgba(255, 255, 248, 0.84);
    font: inherit;
    font-weight: 800;
    text-decoration: none;
    cursor: pointer;
  }

  button {
    margin-top: 18px;
    background: #243126;
    color: #fffdf3;
  }

  .links {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    margin-top: 22px;
  }

  .error {
    margin: 16px 0 0;
    color: #a33a21;
    font-size: 0.92rem;
    line-height: 1.4;
  }

  @media (max-width: 860px) {
    .hero {
      grid-template-columns: 1fr;
      border-radius: 24px;
    }
  }
</style>
