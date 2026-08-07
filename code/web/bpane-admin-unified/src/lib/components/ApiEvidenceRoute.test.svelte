<script lang="ts">
  import type { ApiContractEvidence } from '$lib/api-companion/api-contract-types';
  import ApiEvidenceRoute from './ApiEvidenceRoute.svelte';

  type Props = {
    readonly loadEvidence: () => Promise<ApiContractEvidence>;
  };

  let { loadEvidence }: Props = $props();
</script>

<ApiEvidenceRoute {loadEvidence}>
  {#snippet children(evidence)}
    <p data-testid="api-evidence-ready">{evidence.operations.length} loaded</p>
  {/snippet}
</ApiEvidenceRoute>
