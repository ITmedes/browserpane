<script lang="ts">
  import { page } from '$app/state';
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import SessionObservabilityRoute from '$lib/components/SessionObservabilityRoute.svelte';

  const authContext = useUnifiedAdminContext();
  const sessionId = $derived(page.params.session_id ?? '');
</script>

<SessionObservabilityRoute {authContext} {sessionId} />
