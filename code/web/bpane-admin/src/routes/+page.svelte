<script lang="ts">
  const foundations = [
    {
      title: 'Session Inspector',
      body: 'Owner-scoped session list, lifecycle state, stop blockers, runtime assignment, and live browser entry point.',
    },
    {
      title: 'Operations Timeline',
      body: 'A single timeline for session lifecycle, workflows, recordings, files, and operator interventions.',
    },
    {
      title: 'Runtime Capacity',
      body: 'Docker-pool health, active and starting runtime counts, queue pressure, and stale assignment recovery.',
    },
  ];
</script>

<svelte:head>
  <title>BrowserPane Admin</title>
</svelte:head>

<main class="admin-shell">
  <section class="hero">
    <p class="eyebrow">Reference admin foundation</p>
    <h1>BrowserPane operations without the test-page drift.</h1>
    <p class="lede">
      This package is the future operator console for owner-scoped sessions,
      live browser handoff, recordings, workflow runs, and runtime health.
    </p>
    <div class="actions">
      <a href="/auth-config.json">Auth config</a>
      <a href="/cert-fingerprint">Certificate fingerprint</a>
    </div>
  </section>

  <section class="panel-grid" aria-label="Admin foundation areas">
    {#each foundations as foundation}
      <article>
        <h2>{foundation.title}</h2>
        <p>{foundation.body}</p>
      </article>
    {/each}
  </section>

  <section class="boundary">
    <h2>Implemented in this slice</h2>
    <p>
      The first boundary is a typed control-plane client for the frozen
      <code>/api/v1/sessions</code> API. Authentication, live session embedding,
      and smoke parity will land as separate, testable slices.
    </p>
  </section>
</main>

<style>
  .admin-shell {
    width: min(1120px, calc(100vw - 32px));
    margin: 0 auto;
    padding: 72px 0;
  }

  .hero {
    position: relative;
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

  .eyebrow {
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

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    margin-top: 34px;
  }

  .actions a {
    padding: 12px 16px;
    border: 1px solid rgba(24, 32, 24, 0.18);
    border-radius: 999px;
    color: #162119;
    background: rgba(255, 255, 248, 0.72);
    font-weight: 700;
    text-decoration: none;
  }

  .panel-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 18px;
    margin-top: 22px;
  }

  article,
  .boundary {
    border: 1px solid rgba(24, 32, 24, 0.12);
    border-radius: 24px;
    background: rgba(255, 255, 248, 0.62);
    box-shadow: 0 18px 48px rgba(24, 32, 24, 0.08);
  }

  article {
    padding: 24px;
  }

  h2 {
    margin: 0 0 12px;
    color: #243126;
    font-size: 1.05rem;
  }

  p {
    color: rgba(24, 32, 24, 0.72);
    line-height: 1.55;
  }

  article p,
  .boundary p {
    margin: 0;
  }

  .boundary {
    margin-top: 18px;
    padding: 24px;
  }

  code {
    padding: 2px 6px;
    border-radius: 7px;
    background: rgba(65, 116, 99, 0.12);
  }

  @media (max-width: 760px) {
    .admin-shell {
      padding: 32px 0;
    }

    .hero {
      border-radius: 24px;
    }

    .panel-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
