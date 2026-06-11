<script lang="ts">
  import { ChevronRight } from '@lucide/svelte';
  import ShellAccountButton from '$lib/components/ShellAccountButton.svelte';
  import ShellLogo from '$lib/components/ShellLogo.svelte';
  import ShellNavigation from '$lib/components/ShellNavigation.svelte';
  import ShellNotificationsButton from '$lib/components/ShellNotificationsButton.svelte';
  import ShellSearch from '$lib/components/ShellSearch.svelte';
</script>

<svelte:head>
  <title>BrowserPane Admin Redesign</title>
</svelte:head>

<main
  class="min-h-screen bg-admin-bg text-admin-ink"
  data-testid="admin-new-shell"
>
  <section class="flex min-h-screen flex-col overflow-hidden bg-admin-paper">
    <header
      class="flex min-h-16 shrink-0 items-center gap-3 border-b border-admin-border bg-admin-panel px-4 sm:px-5"
      data-testid="admin-new-header"
    >
      <ShellLogo />

      <div class="hidden min-w-0 items-center gap-1 text-sm text-admin-muted md:flex">
        <ChevronRight size={15} strokeWidth={1.8} />
        <span class="font-medium text-admin-ink">Dashboard</span>
      </div>

      <ShellSearch />

      <ShellNotificationsButton />
      <ShellAccountButton />
    </header>

    <div class="flex min-h-0 flex-1">
      <ShellNavigation />
      <section class="min-w-0 flex-1 bg-admin-bg" aria-label="Workspace"></section>
    </div>
  </section>
</main>
