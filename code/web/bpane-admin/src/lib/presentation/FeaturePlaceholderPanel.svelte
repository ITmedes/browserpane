<script lang="ts">
  import type { AdminFeaturePanelViewModel } from './admin-workspace-view-model';

  type FeaturePlaceholderPanelProps = {
    readonly panel: AdminFeaturePanelViewModel;
  };

  let { panel }: FeaturePlaceholderPanelProps = $props();
</script>

<div class="grid gap-4">
  <p class="m-0 text-sm leading-normal text-admin-ink/68">{panel.description}</p>

  <div class="grid gap-2">
    <p class="admin-eyebrow mb-0">Controls to port from test-embed</p>
    <div class="flex flex-wrap gap-2">
      {#each panel.controls as control}
        <span class="rounded-full border border-admin-ink/10 bg-admin-cream/72 px-3 py-1 text-xs font-bold text-admin-ink/70">
          {control}
        </span>
      {/each}
    </div>
  </div>

  <div class="grid gap-2">
    <p class="admin-eyebrow mb-0">View model signals</p>
    <div class="grid grid-cols-2 gap-2 max-[760px]:grid-cols-1">
      {#each panel.metrics as metric}
        <span class="rounded-[14px] bg-admin-leaf/10 p-3 text-xs font-bold text-admin-ink/68 uppercase">
          {metric}: planned
        </span>
      {/each}
    </div>
  </div>
</div>
