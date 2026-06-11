<script lang="ts">
  import { Bell } from '@lucide/svelte';

  export let label = 'Notifications';
</script>

<button
  class="inline-flex h-9 w-9 items-center justify-center rounded-md border border-admin-border bg-admin-panel text-admin-muted shadow-sm"
  type="button"
  aria-label={label}
  data-testid="admin-new-notifications"
>
  <Bell size={17} strokeWidth={1.8} />
</button>
