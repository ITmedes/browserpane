<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import CredentialBindingDetailRoute from '$lib/components/CredentialBindingDetailRoute.svelte';
  const authContext = useUnifiedAdminContext();
</script>

<CredentialBindingDetailRoute {authContext} />
