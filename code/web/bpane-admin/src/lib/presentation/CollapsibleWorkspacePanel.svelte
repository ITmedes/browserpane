<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { AdminFeaturePanelViewModel } from './admin-workspace-view-model';

  type CollapsibleWorkspacePanelProps = {
    readonly panel: AdminFeaturePanelViewModel;
    readonly open: boolean;
    readonly onToggle: (panelId: AdminFeaturePanelViewModel['id']) => void;
    readonly children?: Snippet;
  };

  let { panel, open, onToggle, children }: CollapsibleWorkspacePanelProps = $props();
</script>

<section
  class="rounded-[24px] border border-admin-ink/12 bg-admin-panel/70 shadow-[0_18px_48px_rgb(24_32_24_/_8%)]"
  data-testid={`workspace-panel-${panel.id}`}
>
  <button
    class="grid w-full cursor-pointer grid-cols-[minmax(0,1fr)_auto] items-center gap-3 px-4 py-3.5 text-left"
    type="button"
    aria-expanded={open}
    data-testid={`workspace-panel-toggle-${panel.id}`}
    onclick={() => onToggle(panel.id)}
  >
    <span class="min-w-0">
      <span class="block text-[0.68rem] font-extrabold tracking-[0.14em] text-admin-leaf uppercase">
        {panel.label}
      </span>
      <span class="mt-1 block overflow-hidden text-ellipsis whitespace-nowrap text-sm font-bold text-admin-night">
        {panel.title}
      </span>
    </span>
    <span class="rounded-full bg-admin-leaf/10 px-2.5 py-1 text-[0.72rem] font-extrabold text-admin-leaf">
      {open ? 'Hide' : panel.status}
    </span>
  </button>

  {#if open}
    <div class="border-t border-admin-ink/10 px-4 py-4">
      {@render children?.()}
    </div>
  {/if}
</section>
