<script lang="ts">
  import {
    Bell,
    ChevronRight,
    CircleUserRound,
    Search,
  } from '@lucide/svelte';
  import ShellNavigation from '$lib/components/ShellNavigation.svelte';
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
      <div class="flex min-w-0 items-center gap-3">
        <span
          class="inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-md bg-admin-shell text-[11px] font-bold text-white"
          aria-label="BrowserPane"
        >
          BP
        </span>
        <div class="min-w-0">
          <div class="truncate text-sm font-semibold text-admin-ink">BrowserPane</div>
          <div class="hidden truncate font-mono text-xs text-admin-muted sm:block">Unified admin</div>
        </div>
      </div>

      <div class="hidden min-w-0 items-center gap-1 text-sm text-admin-muted md:flex">
        <ChevronRight size={15} strokeWidth={1.8} />
        <span class="font-medium text-admin-ink">Dashboard</span>
      </div>

      <button
        class="ml-auto hidden h-9 min-w-72 items-center gap-2 rounded-md border border-admin-border bg-admin-bg px-3 text-left text-sm text-admin-muted shadow-sm lg:flex"
        type="button"
      >
        <Search size={16} strokeWidth={1.8} />
        <span class="min-w-0 flex-1 truncate">Search sessions, workflows, resources</span>
      </button>

      <button
        class="inline-flex h-9 w-9 items-center justify-center rounded-md border border-admin-border bg-admin-panel text-admin-muted shadow-sm"
        type="button"
        aria-label="Notifications"
      >
        <Bell size={17} strokeWidth={1.8} />
      </button>

      <button
        class="inline-flex h-9 w-9 items-center justify-center rounded-md border border-admin-border bg-admin-panel text-admin-muted shadow-sm"
        type="button"
        aria-label="Account"
      >
        <CircleUserRound size={18} strokeWidth={1.8} />
      </button>
    </header>

    <div class="flex min-h-0 flex-1">
      <ShellNavigation />
      <section class="min-w-0 flex-1 bg-admin-bg" aria-label="Workspace"></section>
    </div>
  </section>
</main>
