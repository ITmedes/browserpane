<script lang="ts">
  import {
    Activity,
    BookOpen,
    Boxes,
    Folder,
    Gauge,
    Layers3,
    Monitor,
    Network,
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
