<svelte:head>
  <title>BrowserPane Admin Redesign</title>
</svelte:head>

<main
  class="grid min-h-screen place-items-center bg-admin-bg px-6 py-10 text-admin-ink"
  data-testid="admin-new-shell"
>
  <section class="grid max-w-2xl gap-4 rounded-lg border border-admin-border bg-admin-panel p-8 shadow-sm">
    <p class="m-0 font-mono text-xs font-semibold uppercase text-admin-muted">
      /admin-new
    </p>
    <h1 class="m-0 text-3xl font-semibold">BrowserPane Admin Redesign</h1>
    <p class="m-0 text-base leading-7 text-admin-muted">
      Empty SvelteKit shell for the route-backed admin app migration. The current
      admin app remains available at /admin/.
    </p>
  </section>
</main>
