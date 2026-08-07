<script lang="ts">
  import { page } from '$app/state';
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import SessionFilesRoute from '$lib/components/SessionFilesRoute.svelte';

  const authContext = useUnifiedAdminContext();
  const sessionId = $derived(page.params.session_id ?? '');
</script>

<SessionFilesRoute {authContext} {sessionId} />
