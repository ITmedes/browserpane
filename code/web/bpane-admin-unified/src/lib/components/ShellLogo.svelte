<script lang="ts">
  export let subtitle = 'Unified admin';
</script>

<a
  class="flex min-w-0 items-center gap-3"
  href="/admin-new/"
  aria-label="BrowserPane admin home"
  data-testid="admin-new-logo"
>
  <span
    class="inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-md bg-admin-shell text-[11px] font-bold text-white"
    aria-hidden="true"
  >
    BP
  </span>
  <span class="min-w-0">
    <span class="block truncate text-sm font-semibold text-admin-ink">BrowserPane</span>
    <span class="hidden truncate font-mono text-xs text-admin-muted sm:block">{subtitle}</span>
  </span>
</a>
