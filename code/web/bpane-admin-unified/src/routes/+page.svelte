<script lang="ts">
  import {
    Activity,
    Bell,
    BookOpen,
    Boxes,
    ChevronRight,
    CircleUserRound,
    Folder,
    Gauge,
    Layers3,
    Monitor,
    Network,
    Search,
    ShieldCheck,
    SquareTerminal,
  } from '@lucide/svelte';
  import type { Component } from 'svelte';
  import { primaryNav, secondaryNav, type NavIconKey } from '$lib/admin-navigation';

  const navIcons: Record<NavIconKey, Component<{ size?: number; strokeWidth?: number; class?: string }>> = {
    activity: Activity,
    'book-open': BookOpen,
    boxes: Boxes,
    folder: Folder,
    gauge: Gauge,
    layers: Layers3,
    monitor: Monitor,
    network: Network,
    shield: ShieldCheck,
    terminal: SquareTerminal,
  };
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
      <aside
        class="hidden w-64 shrink-0 flex-col border-r border-admin-border bg-admin-panel md:flex"
        data-testid="admin-new-side-nav"
      >
        <nav class="min-h-0 flex-1 overflow-y-auto px-3 py-4" aria-label="Primary">
          <div class="space-y-1">
            {#each primaryNav as item}
              {@const Icon = navIcons[item.icon]}
              <a
                class={`flex h-10 items-center gap-3 rounded-md px-3 text-sm font-medium ${
                  item.active
                    ? 'bg-[#eef2ff] text-admin-accent'
                    : 'text-admin-muted hover:bg-admin-soft hover:text-admin-ink'
                }`}
                href={item.route}
              >
                <Icon size={16} strokeWidth={1.9} />
                <span class="truncate">{item.label}</span>
              </a>
            {/each}
          </div>

          <div class="mt-6 border-t border-admin-border pt-4">
            <div class="px-3 pb-2 text-xs font-semibold text-admin-muted">Govern</div>
            <div class="space-y-1">
              {#each secondaryNav as item}
                {@const Icon = navIcons[item.icon]}
                <a
                  class="flex h-10 items-center gap-3 rounded-md px-3 text-sm font-medium text-admin-muted hover:bg-admin-soft hover:text-admin-ink"
                  href={item.route}
                >
                  <Icon size={16} strokeWidth={1.9} />
                  <span class="truncate">{item.label}</span>
                </a>
              {/each}
            </div>
          </div>
        </nav>

        <div class="border-t border-admin-border p-3">
          <div class="rounded-md border border-admin-border bg-admin-bg p-3">
            <div class="flex items-center gap-2 text-sm font-medium text-admin-ink">
              <span class="h-2 w-2 rounded-full bg-admin-success"></span>
              Local stack
            </div>
            <div class="mt-1 font-mono text-xs text-admin-muted">docker_pool</div>
          </div>
        </div>
      </aside>

      <nav
        class="flex w-16 shrink-0 flex-col items-center gap-2 border-r border-admin-border bg-admin-panel py-3 md:hidden"
        aria-label="Primary"
      >
        {#each primaryNav.slice(0, 6) as item}
          {@const Icon = navIcons[item.icon]}
          <a
            class={`inline-flex h-10 w-10 items-center justify-center rounded-md ${
              item.active ? 'bg-[#eef2ff] text-admin-accent' : 'text-admin-muted'
            }`}
            href={item.route}
            aria-label={item.label}
          >
            <Icon size={17} strokeWidth={1.9} />
          </a>
        {/each}
      </nav>

      <section class="min-w-0 flex-1 bg-admin-bg" aria-label="Workspace"></section>
    </div>
  </section>
</main>
