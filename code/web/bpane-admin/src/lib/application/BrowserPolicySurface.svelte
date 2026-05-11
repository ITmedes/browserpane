<script lang="ts">
  import { onDestroy } from 'svelte';
  import type { SessionResource } from '../api/control-types';
  import BrowserPolicyPanel from '../presentation/BrowserPolicyPanel.svelte';
  import { BrowserPolicyViewModelBuilder } from '../presentation/browser-policy-view-model';

  type BrowserPolicySurfaceProps = {
    readonly selectedSession: SessionResource | null;
    readonly onRefreshSelectedSession: () => Promise<void>;
  };

  let { selectedSession, onRefreshSelectedSession }: BrowserPolicySurfaceProps = $props();
  let copied = $state(false);
  let copyTimer: ReturnType<typeof setTimeout> | null = null;
  const viewModel = $derived(BrowserPolicyViewModelBuilder.build(selectedSession));

  onDestroy(() => {
    if (copyTimer) {
      clearTimeout(copyTimer);
    }
  });

  async function copyProbeCommand(): Promise<void> {
    if (!viewModel.probeCommand) {
      return;
    }
    await navigator.clipboard.writeText(viewModel.probeCommand);
    copied = true;
    if (copyTimer) {
      clearTimeout(copyTimer);
    }
    copyTimer = setTimeout(() => {
      copied = false;
      copyTimer = null;
    }, 1600);
  }
</script>

<BrowserPolicyPanel
  {viewModel}
  {copied}
  onRefresh={() => void onRefreshSelectedSession()}
  onCopyProbeCommand={() => void copyProbeCommand()}
/>
