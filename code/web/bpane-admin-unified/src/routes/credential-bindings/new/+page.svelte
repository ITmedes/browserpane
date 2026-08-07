<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import CredentialBindingCreateRoute from '$lib/components/CredentialBindingCreateRoute.svelte';
  const authContext = useUnifiedAdminContext();
</script>

<CredentialBindingCreateRoute {authContext} />
