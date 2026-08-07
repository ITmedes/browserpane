<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import IdentityAccessReviewRoute from '$lib/components/IdentityAccessReviewRoute.svelte';

  const authContext = useUnifiedAdminContext();
</script>

<IdentityAccessReviewRoute {authContext} />
