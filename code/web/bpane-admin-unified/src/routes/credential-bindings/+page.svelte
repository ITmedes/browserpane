<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import CredentialBindingOverviewRoute from '$lib/components/CredentialBindingOverviewRoute.svelte';
  const authContext = useUnifiedAdminContext();
</script>

<CredentialBindingOverviewRoute {authContext} />
