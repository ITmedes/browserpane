<script lang="ts">
  import ApiCoverageWorkspace from '$lib/components/ApiCoverageWorkspace.svelte';
  import ApiEvidenceRoute from '$lib/components/ApiEvidenceRoute.svelte';
</script>

<ApiEvidenceRoute>
  {#snippet children(evidence)}
    <ApiCoverageWorkspace {evidence} />
  {/snippet}
</ApiEvidenceRoute>
