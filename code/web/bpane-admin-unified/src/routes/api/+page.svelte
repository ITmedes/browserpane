<script lang="ts">
  import ApiCompanionWorkspace from '$lib/components/ApiCompanionWorkspace.svelte';
  import ApiEvidenceRoute from '$lib/components/ApiEvidenceRoute.svelte';
</script>

<ApiEvidenceRoute>
  {#snippet children(evidence)}
    <ApiCompanionWorkspace {evidence} />
  {/snippet}
</ApiEvidenceRoute>
