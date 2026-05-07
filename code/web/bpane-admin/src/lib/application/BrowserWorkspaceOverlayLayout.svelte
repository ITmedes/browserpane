<script lang="ts">
  import type { Snippet } from 'svelte';
  import { fly } from 'svelte/transition';

  type BrowserWorkspaceOverlayLayoutProps = {
    readonly browser: Snippet;
    readonly admin: Snippet;
    readonly adminOpen: boolean;
    readonly onAdminOpenChange: (open: boolean) => void;
  };

  let { browser, admin, adminOpen, onAdminOpenChange }: BrowserWorkspaceOverlayLayoutProps = $props();
</script>

<section class="relative min-h-[calc(100vh-76px)]">
  <main class="min-w-0">
    {@render browser()}
  </main>

  {#if adminOpen}
    <aside
      class="fixed top-[64px] right-2 bottom-2 z-40 w-[min(560px,calc(100vw-16px))] overflow-hidden rounded-2xl border border-[#90a6cc]/20 bg-[#0e1829]/95 shadow-[0_24px_64px_rgb(0_0_0_/_34%)] backdrop-blur-xl sm:top-[72px] sm:right-4 sm:bottom-4"
      data-testid="admin-overlay"
      transition:fly={{ x: 42, duration: 160 }}
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
  {/if}
</section>
