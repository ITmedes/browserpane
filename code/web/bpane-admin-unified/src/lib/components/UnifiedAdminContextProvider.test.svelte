<script lang="ts">
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import UnifiedAdminContextProbe from './UnifiedAdminContextProbe.test.svelte';
  import UnifiedAdminContextProvider from './UnifiedAdminContextProvider.svelte';

  let { authContext }: { readonly authContext: UnifiedAdminContext } = $props();
</script>

<UnifiedAdminContextProvider {authContext}>
  <UnifiedAdminContextProbe />
</UnifiedAdminContextProvider>
