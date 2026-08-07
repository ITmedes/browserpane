<script lang="ts">
  import { page } from '$app/state';
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import SessionAutomationRoute from '$lib/components/SessionAutomationRoute.svelte';

  const authContext = useUnifiedAdminContext();
  const sessionId = $derived(page.params.session_id ?? '');
</script>

<SessionAutomationRoute {authContext} {sessionId} />
