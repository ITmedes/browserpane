<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';

  const authContext = useUnifiedAdminContext();
</script>

<div data-testid="unified-admin-context-probe">
  {authContext.auth.username}
</div>
