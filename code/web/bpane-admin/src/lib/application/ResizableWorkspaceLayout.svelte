<script lang="ts">
  import type { Snippet } from 'svelte';

  type ResizableWorkspaceLayoutProps = {
    readonly browser: Snippet;
    readonly sidebar: Snippet;
  };

  const MIN_SIDEBAR_WIDTH = 340;
  const DEFAULT_SIDEBAR_WIDTH = 420;
  const MAX_SIDEBAR_WIDTH = 640;

  let { browser, sidebar }: ResizableWorkspaceLayoutProps = $props();
  let root: HTMLElement | null = null;
  let sidebarWidth = $state(DEFAULT_SIDEBAR_WIDTH);
  let resizing = $state(false);
  const gridStyle = $derived(`--admin-sidebar-width: ${sidebarWidth}px;`);

  function startResize(event: PointerEvent): void {
    if (!root || window.innerWidth < 1280) {
      return;
    }
    resizing = true;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  }

  function resize(event: PointerEvent): void {
    if (!resizing || !root) {
      return;
    }
    const rect = root.getBoundingClientRect();
    sidebarWidth = clamp(rect.right - event.clientX, MIN_SIDEBAR_WIDTH, maxSidebarWidth(rect.width));
  }

  function stopResize(event: PointerEvent): void {
    if (!resizing) {
      return;
    }
    resizing = false;
    (event.currentTarget as HTMLElement).releasePointerCapture(event.pointerId);
  }

  function maxSidebarWidth(containerWidth: number): number {
    return Math.min(MAX_SIDEBAR_WIDTH, Math.max(MIN_SIDEBAR_WIDTH, containerWidth * 0.42));
  }

  function clamp(value: number, min: number, max: number): number {
    return Math.round(Math.min(Math.max(value, min), max));
  }
</script>

<section
  class="grid gap-5 xl:grid-cols-[minmax(0,1fr)_12px_var(--admin-sidebar-width)]"
  style={gridStyle}
  bind:this={root}
>
  <main class="min-w-0 xl:sticky xl:top-[72px] xl:self-start">
    {@render browser()}
  </main>

  <button
    class={`hidden cursor-col-resize rounded-full border border-admin-ink/10 bg-admin-ink/10 transition-colors xl:block ${
      resizing ? 'bg-admin-leaf/35' : 'hover:bg-admin-leaf/20'
    }`}
    type="button"
    aria-label="Resize admin sidebar"
    data-testid="workspace-resize-handle"
    onpointerdown={startResize}
    onpointermove={resize}
    onpointerup={stopResize}
    onpointercancel={stopResize}
  ></button>

  <aside class="min-w-0">
    {@render sidebar()}
  </aside>
</section>
