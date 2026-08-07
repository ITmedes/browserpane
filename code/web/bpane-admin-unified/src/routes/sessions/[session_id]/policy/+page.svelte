<script lang="ts">
  import { page } from '$app/state';
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import SessionPolicyRoute from '$lib/components/SessionPolicyRoute.svelte';

  const authContext = useUnifiedAdminContext();
  const sessionId = $derived(page.params.session_id ?? '');
</script>

<SessionPolicyRoute {authContext} {sessionId} />
