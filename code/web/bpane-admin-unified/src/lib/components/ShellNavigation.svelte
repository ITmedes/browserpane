<script lang="ts">
  import {
    Activity,
    BookOpen,
    Boxes,
    Check,
    Cpu,
    Folder,
    Gauge,
    Globe,
    Info,
    KeyRound,
    Layers3,
    List,
    Monitor,
    Network,
    ShieldCheck,
    SquareTerminal,
    Workflow,
  } from '@lucide/svelte';
  import type { Component } from 'svelte';
  import { navGroups, primaryNav, type NavIconKey } from '$lib/admin-navigation';

  type ShellNavigationProps = {
    readonly activeId?: string;
  };

  let { activeId = 'dashboard' }: ShellNavigationProps = $props();

  const navIcons: Record<NavIconKey, Component<{ size?: number; strokeWidth?: number; class?: string }>> = {
    activity: Activity,
    'book-open': BookOpen,
    boxes: Boxes,
    check: Check,
    cpu: Cpu,
    folder: Folder,
    gauge: Gauge,
    globe: Globe,
    info: Info,
    key: KeyRound,
    layers: Layers3,
    list: List,
    monitor: Monitor,
    network: Network,
    shield: ShieldCheck,
    terminal: SquareTerminal,
    workflow: Workflow,
  };

  function isActive(itemId: string): boolean {
    return itemId === activeId;
  }
</script>

<aside
  class="hidden w-64 shrink-0 flex-col border-r border-admin-border bg-admin-panel md:flex"
  data-testid="admin-new-side-nav"
>
  <nav class="min-h-0 flex-1 overflow-y-auto px-3 py-4" aria-label="Primary">
    <div class="space-y-4">
      {#each navGroups as group}
        <div class="space-y-1">
          {#if group.group}
            <div
              class="px-3 pb-1 pt-2 text-xs font-semibold uppercase tracking-[0.1em] text-admin-muted"
              data-testid="admin-new-nav-group"
            >
              {group.group}
            </div>
          {/if}
          {#each group.items as item}
            {@const Icon = navIcons[item.icon]}
            <a
              class={`flex h-10 items-center gap-3 rounded-md px-3 text-sm font-medium ${
                isActive(item.id)
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
      {/each}
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
        isActive(item.id) ? 'bg-[#eef2ff] text-admin-accent' : 'text-admin-muted'
      }`}
      href={item.route}
      aria-label={item.label}
    >
      <Icon size={17} strokeWidth={1.9} />
    </a>
  {/each}
</nav>
