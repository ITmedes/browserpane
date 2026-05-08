<script lang="ts">
  import type { Snippet } from 'svelte';
  type BrowserWorkspaceOverlayLayoutProps = {
    readonly admin: Snippet;
    readonly adminOpen: boolean;
    readonly onAdminOpenChange: (open: boolean) => void;
  };

  let { admin, adminOpen, onAdminOpenChange }: BrowserWorkspaceOverlayLayoutProps = $props();
</script>

<aside
  class={`fixed top-[72px] right-2 bottom-2 z-40 w-[min(560px,calc(100vw-16px))] overflow-hidden rounded-2xl border border-[#90a6cc]/20 bg-[#0e1829]/95 shadow-2xl shadow-black/45 ring-1 ring-white/10 backdrop-blur-xl transition-[opacity,transform] duration-150 sm:top-20 sm:right-4 sm:bottom-4 ${
    adminOpen ? 'translate-x-0 opacity-100' : 'pointer-events-none translate-x-[calc(100%+32px)] opacity-0'
  }`}
  data-testid="admin-overlay"
  data-admin-open={adminOpen ? 'true' : 'false'}
  aria-hidden={!adminOpen}
>
  <div class="flex min-h-14 items-center justify-between gap-3 border-b border-[#90a6cc]/18 px-4 py-3">
    <div class="min-w-0">
      <p class="admin-eyebrow mb-1">Admin</p>
      <h2 class="m-0 overflow-hidden text-ellipsis whitespace-nowrap text-sm font-bold text-admin-ink">
        Operations overlay
      </h2>
    </div>
    <button class="rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 py-1.5 text-xs font-bold text-admin-ink" type="button" data-testid="admin-overlay-close" onclick={() => onAdminOpenChange(false)}>
      Hide
    </button>
  </div>
  <div class="h-[calc(100%-64px)] min-h-0">
    {@render admin()}
  </div>
</aside>
