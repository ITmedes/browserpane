<script lang="ts">
  import {
    Activity,
    Bell,
    BookOpen,
    Boxes,
    ChevronRight,
    CircleUserRound,
    Command,
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
  class="min-h-screen bg-admin-shell p-0 text-admin-ink sm:p-4 lg:p-6"
  data-testid="admin-new-shell"
>
  <section
    class="flex min-h-screen flex-col overflow-hidden bg-admin-paper shadow-2xl sm:min-h-[calc(100vh-2rem)] sm:rounded-xl lg:min-h-[calc(100vh-3rem)]"
  >
    <div
      class="hidden h-8 shrink-0 items-center gap-2 border-b border-admin-border bg-[#f4f5f7] px-3 sm:flex"
      aria-hidden="true"
    >
      <span class="h-2.5 w-2.5 rounded-full bg-[#ff5f57]"></span>
      <span class="h-2.5 w-2.5 rounded-full bg-[#febc2e]"></span>
      <span class="h-2.5 w-2.5 rounded-full bg-[#28c840]"></span>
      <div
        class="mx-auto flex h-5 w-full max-w-xl items-center justify-center rounded border border-admin-border bg-white px-3 font-mono text-[11px] text-admin-muted"
      >
        app.browserpane.io/admin-new
      </div>
      <div class="w-12"></div>
    </div>

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

      <section class="min-w-0 flex-1 overflow-auto bg-admin-bg">
        <div class="mx-auto grid max-w-6xl gap-4 p-4 sm:p-6">
          <div class="flex flex-wrap items-end justify-between gap-3">
            <div>
              <p class="m-0 font-mono text-xs font-medium text-admin-muted">/admin-new</p>
              <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink sm:text-3xl">Dashboard</h1>
            </div>
            <button
              class="inline-flex h-9 items-center gap-2 rounded-md bg-admin-accent px-3 text-sm font-semibold text-white shadow-sm"
              type="button"
            >
              <Command size={15} strokeWidth={2} />
              Command
            </button>
          </div>

          <div class="grid gap-4 lg:grid-cols-[1.45fr_0.85fr]">
            <article class="min-h-72 rounded-lg border border-admin-border bg-admin-panel p-5 shadow-sm">
              <div class="flex items-center justify-between gap-4">
                <div>
                  <h2 class="m-0 text-base font-semibold text-admin-ink">Live workspace</h2>
                  <p class="m-0 mt-1 text-sm text-admin-muted">No session selected</p>
                </div>
                <span class="rounded-md border border-admin-border bg-admin-bg px-2.5 py-1 font-mono text-xs text-admin-muted">
                  standby
                </span>
              </div>

              <div class="mt-6 grid min-h-44 place-items-center rounded-lg border border-dashed border-admin-border bg-admin-bg">
                <div class="text-center">
                  <Monitor class="mx-auto text-admin-muted" size={28} strokeWidth={1.6} />
                  <div class="mt-3 text-sm font-medium text-admin-ink">No live preview</div>
                </div>
              </div>
            </article>

            <article class="rounded-lg border border-admin-border bg-admin-panel p-5 shadow-sm">
              <h2 class="m-0 text-base font-semibold text-admin-ink">Resource focus</h2>
              <div class="mt-4 space-y-3">
                <div class="rounded-md border border-admin-border bg-admin-bg p-3">
                  <div class="text-sm font-medium text-admin-ink">Sessions</div>
                  <div class="mt-1 text-sm text-admin-muted">0 connected, 0 running</div>
                </div>
                <div class="rounded-md border border-admin-border bg-admin-bg p-3">
                  <div class="text-sm font-medium text-admin-ink">Operations</div>
                  <div class="mt-1 text-sm text-admin-muted">No active operation</div>
                </div>
                <div class="rounded-md border border-admin-border bg-admin-bg p-3">
                  <div class="text-sm font-medium text-admin-ink">Governance</div>
                  <div class="mt-1 text-sm text-admin-muted">Local policy profile</div>
                </div>
              </div>
            </article>
          </div>
        </div>
      </section>
    </div>
  </section>
</main>
