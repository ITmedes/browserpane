<script lang="ts">
  import type { Snippet } from 'svelte';
  import { provideUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';

  type UnifiedAdminContextProviderProps = {
    readonly authContext: UnifiedAdminContext;
    readonly children?: Snippet;
  };

  let { authContext, children }: UnifiedAdminContextProviderProps = $props();
  // svelte-ignore state_referenced_locally
  provideUnifiedAdminContext(authContext);
</script>

{@render children?.()}
