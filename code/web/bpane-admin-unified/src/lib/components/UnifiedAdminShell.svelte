<script lang="ts">
  import type { Snippet } from 'svelte';
  import ShellAccountButton from '$lib/components/ShellAccountButton.svelte';
  import ShellLogo from '$lib/components/ShellLogo.svelte';
  import ShellNavigation from '$lib/components/ShellNavigation.svelte';
  import ShellNotificationsButton from '$lib/components/ShellNotificationsButton.svelte';
  import ShellSearch from '$lib/components/ShellSearch.svelte';

  type UnifiedAdminShellProps = {
    readonly activeId?: string;
    readonly title?: string;
    readonly children?: Snippet;
  };

  let {
    activeId = 'dashboard',
    title = 'BrowserPane Admin Redesign',
    children,
  }: UnifiedAdminShellProps = $props();
</script>

<svelte:head>
  <title>{title}</title>
</svelte:head>

<main class="min-h-screen bg-admin-bg text-admin-ink" data-testid="admin-new-shell">
  <section class="flex min-h-screen flex-col overflow-hidden bg-admin-paper">
    <header
      class="flex min-h-16 shrink-0 items-center gap-3 border-b border-admin-border bg-admin-panel px-4 sm:px-5"
      data-testid="admin-new-header">
      <ShellLogo />
      <ShellSearch />
      <ShellNotificationsButton />
      <ShellAccountButton />
    </header>

    <div class="flex min-h-0 flex-1">
      <ShellNavigation {activeId} />
      <section class="min-w-0 flex-1 overflow-y-auto bg-admin-bg" aria-label="Workspace">
        {#if children}
          {@render children()}
        {/if}
      </section>
    </div>
  </section>
</main>
