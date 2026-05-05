<script lang="ts">
  import type { Snippet } from 'svelte';
  import { fly } from 'svelte/transition';

  type BrowserWorkspaceOverlayLayoutProps = {
    readonly browser: Snippet;
    readonly admin: Snippet;
  };

  let { browser, admin }: BrowserWorkspaceOverlayLayoutProps = $props();
  let adminOpen = $state(true);
</script>

<section class="relative min-h-[calc(100vh-76px)]">
  <main class="min-w-0">
    {@render browser()}
  </main>

  {#if adminOpen}
    <aside
      class="fixed top-[72px] right-4 bottom-4 z-40 w-[min(540px,calc(100vw-24px))] overflow-hidden rounded-[28px] border border-admin-ink/14 bg-admin-panel/92 shadow-[0_28px_90px_rgb(24_32_24_/_24%)] backdrop-blur-xl"
      data-testid="admin-overlay"
      transition:fly={{ x: 42, duration: 160 }}
    >
      <div class="flex h-14 items-center justify-between gap-3 border-b border-admin-ink/10 px-4">
        <div class="min-w-0">
          <p class="admin-eyebrow mb-0">Admin</p>
          <h2 class="m-0 overflow-hidden text-ellipsis whitespace-nowrap text-sm font-extrabold text-admin-night">
            Operations overlay
          </h2>
        </div>
        <button class="admin-header-button" type="button" data-testid="admin-overlay-close" onclick={() => { adminOpen = false; }}>
          Hide
        </button>
      </div>
      <div class="h-[calc(100%-56px)] min-h-0">
        {@render admin()}
      </div>
    </aside>
  {:else}
    <button
      class="fixed top-[76px] right-4 z-40 rounded-full border border-admin-ink/14 bg-admin-night px-4 py-2 text-sm font-extrabold text-admin-cream shadow-[0_14px_34px_rgb(24_32_24_/_20%)]"
      type="button"
      data-testid="admin-overlay-open"
      onclick={() => { adminOpen = true; }}
    >
      Admin
    </button>
  {/if}
</section>
