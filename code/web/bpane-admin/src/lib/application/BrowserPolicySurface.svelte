<script lang="ts">
  import { onDestroy } from 'svelte';
  import type { SessionResource } from '../api/control-types';
  import type { AdminMessageFeedback } from '../presentation/admin-message-types';
  import BrowserPolicyPanel from '../presentation/BrowserPolicyPanel.svelte';
  import { BrowserPolicyViewModelBuilder } from '../presentation/browser-policy-view-model';

  type BrowserPolicySurfaceProps = {
    readonly selectedSession: SessionResource | null;
    readonly onRefreshSelectedSession: () => Promise<void>;
  };

  let { selectedSession, onRefreshSelectedSession }: BrowserPolicySurfaceProps = $props();
  let currentSessionId = $state<string | null>(null);
  let copied = $state(false);
  let feedback = $state<AdminMessageFeedback | null>(null);
  let copyTimer: ReturnType<typeof setTimeout> | null = null;
  const viewModel = $derived(BrowserPolicyViewModelBuilder.build(selectedSession));

  $effect(() => {
    const nextSessionId = selectedSession?.id ?? null;
    if (nextSessionId === currentSessionId) {
      return;
    }
    currentSessionId = nextSessionId;
    copied = false;
    feedback = null;
    if (copyTimer) {
      clearTimeout(copyTimer);
      copyTimer = null;
    }
  });

  onDestroy(() => {
    if (copyTimer) {
      clearTimeout(copyTimer);
    }
  });

  async function copyProbeCommand(): Promise<void> {
    if (!viewModel.probeCommand) {
      return;
    }
    try {
      await navigator.clipboard.writeText(viewModel.probeCommand);
      copied = true;
      feedback = {
        variant: 'success',
        title: 'Policy command copied',
        message: 'CDP local-file policy probe command copied.',
        testId: 'policy-message',
      };
      if (copyTimer) {
        clearTimeout(copyTimer);
      }
      copyTimer = setTimeout(() => {
        copied = false;
        copyTimer = null;
      }, 1600);
    } catch (error) {
      feedback = {
        variant: 'error',
        title: 'Copy failed',
        message: error instanceof Error ? error.message : 'Could not copy policy probe command.',
        testId: 'policy-message',
      };
    }
  }

  async function refreshPolicy(): Promise<void> {
    feedback = null;
    try {
      await onRefreshSelectedSession();
      feedback = {
        variant: 'success',
        title: 'Policy refreshed',
        message: 'Selected session policy data refreshed.',
        testId: 'policy-message',
      };
    } catch (error) {
      feedback = {
        variant: 'error',
        title: 'Policy refresh failed',
        message: error instanceof Error ? error.message : 'Could not refresh selected session policy data.',
        testId: 'policy-message',
      };
    }
  }
</script>

<BrowserPolicyPanel
  {viewModel}
  {copied}
  {feedback}
  onRefresh={() => void refreshPolicy()}
  onCopyProbeCommand={() => void copyProbeCommand()}
/>
