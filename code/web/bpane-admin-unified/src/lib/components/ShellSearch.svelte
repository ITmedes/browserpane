<script lang="ts">
  import { Search } from '@lucide/svelte';

  export let placeholder = 'Search sessions, workflows, resources';
</script>

<button
  class="ml-auto hidden h-9 min-w-72 items-center gap-2 rounded-md border border-admin-border bg-admin-bg px-3 text-left text-sm text-admin-muted shadow-sm lg:flex"
  type="button"
  data-testid="admin-new-search"
>
  <Search size={16} strokeWidth={1.8} />
  <span class="min-w-0 flex-1 truncate">{placeholder}</span>
</button>
