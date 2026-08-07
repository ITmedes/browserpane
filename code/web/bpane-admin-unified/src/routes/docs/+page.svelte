<script lang="ts">
  import AdminDocsWorkspace from '$lib/components/AdminDocsWorkspace.svelte';
  import ApiEvidenceRoute from '$lib/components/ApiEvidenceRoute.svelte';
</script>

<ApiEvidenceRoute>
  {#snippet children(evidence)}
    <AdminDocsWorkspace {evidence} />
  {/snippet}
</ApiEvidenceRoute>
