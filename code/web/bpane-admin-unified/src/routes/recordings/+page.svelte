<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import RecordingOverviewRoute from '$lib/components/RecordingOverviewRoute.svelte';

  const authContext = useUnifiedAdminContext();
</script>

<RecordingOverviewRoute {authContext} />
