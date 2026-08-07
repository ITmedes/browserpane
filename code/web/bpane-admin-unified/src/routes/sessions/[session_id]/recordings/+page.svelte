<script lang="ts">
  import { page } from '$app/state';
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import SessionRecordingsRoute from '$lib/components/SessionRecordingsRoute.svelte';

  const authContext = useUnifiedAdminContext();
  const sessionId = $derived(page.params.session_id ?? '');
</script>

<SessionRecordingsRoute {authContext} {sessionId} />
