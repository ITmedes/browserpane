<script lang="ts">
  import { Copy, RefreshCw, ShieldCheck, ShieldOff } from '@lucide/svelte';
  import type { AdminActionState } from '$lib/application/admin-async-state';
  import {
    createExtensionVersionDraft,
    labelSummary,
    validateExtensionVersionDraft,
    type ExtensionDetailLoadState,
    type ExtensionVersionDraft,
  } from '$lib/extensions/extension-view-model';
  import type { CreateExtensionVersionRequest } from '$lib/extensions/extension-types';
  import { formatDateTime } from '$lib/projects/project-formatters';
  import { projectToneClass } from '$lib/projects/project-ui';
  import ActionFeedback from './ActionFeedback.svelte';
  import AdminMessage from './AdminMessage.svelte';

  let {
    state: loadState,
    actionState = { status: 'idle' },
    onRefresh,
    onSetEnabled,
    onPublishVersion,
  }: {
    readonly state: ExtensionDetailLoadState;
    readonly actionState?: AdminActionState;
    readonly onRefresh?: () => void | Promise<void>;
    readonly onSetEnabled?: (enabled: boolean) => void | Promise<void>;
    readonly onPublishVersion?: (request: CreateExtensionVersionRequest) => void | Promise<void>;
  } = $props();
  let versionDraft = $state<ExtensionVersionDraft>(createExtensionVersionDraft());
  const versionValidation = $derived(validateExtensionVersionDraft(versionDraft));
  const busy = $derived(actionState.status === 'running');

  function publishVersion(): void {
    if (!versionValidation.request) return;
    void onPublishVersion?.(versionValidation.request);
  }
</script>

<aside class="min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="extension-inspector">
  {#if loadState.status === 'idle'}
    <div class="flex min-h-64 items-center justify-center p-6 text-sm text-admin-muted">Select an extension to inspect.</div>
  {:else if loadState.status === 'loading'}
    <div class="flex min-h-64 items-center justify-center p-6 text-sm text-admin-muted" data-testid="extension-inspector-loading">Loading extension...</div>
  {:else if loadState.status === 'error'}
    <div class="p-4"><AdminMessage tone="error" title="Extension unavailable" message={loadState.message} testId="extension-inspector-error" /></div>
  {:else}
    <div class="border-b border-admin-border p-4">
      <div class="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
        <div class="min-w-0">
          <div class="flex flex-wrap items-center gap-2">
            <h2 class="m-0 text-xl font-semibold text-admin-ink" data-testid="extension-detail-name">{loadState.extension.name}</h2>
            <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(loadState.extension.enabled ? 'success' : 'neutral')}`} data-testid="extension-detail-state">{loadState.extension.enabled ? 'Enabled' : 'Disabled'}</span>
          </div>
          <p class="m-0 mt-1 text-sm text-admin-muted">{loadState.extension.description ?? 'No description available.'}</p>
          <p class="m-0 mt-2 truncate font-mono text-xs text-admin-muted">{loadState.extension.id}</p>
        </div>
        <div class="flex flex-wrap gap-2">
          <button class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft" type="button" onclick={() => void navigator.clipboard?.writeText(loadState.extension.id)} data-testid="extension-copy-id"><Copy size={15} strokeWidth={1.8} /><span>Copy ID</span></button>
          <button class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:opacity-60" type="button" onclick={() => void onRefresh?.()} disabled={busy} data-testid="extension-refresh-detail"><RefreshCw size={15} strokeWidth={1.8} /><span>Refresh</span></button>
          <button class={`inline-flex h-9 items-center gap-2 rounded-md border px-3 text-sm font-medium disabled:opacity-60 ${loadState.extension.enabled ? 'border-amber-200 bg-amber-50 text-amber-900 hover:bg-amber-100' : 'border-emerald-200 bg-emerald-50 text-emerald-800 hover:bg-emerald-100'}`} type="button" onclick={() => void onSetEnabled?.(!loadState.extension.enabled)} disabled={busy} data-testid="extension-toggle-enabled">
            {#if loadState.extension.enabled}<ShieldOff size={15} strokeWidth={1.8} /><span>Disable</span>{:else}<ShieldCheck size={15} strokeWidth={1.8} /><span>Enable</span>{/if}
          </button>
        </div>
      </div>
    </div>

    <div class="border-b border-admin-border p-4"><ActionFeedback state={actionState} successTitle="Extension action completed" errorTitle="Extension action failed" successTestId="extension-action-success" errorTestId="extension-action-error" runningTestId="extension-action-running" /></div>

    <div class="grid gap-4 p-4">
      <section class="rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="extension-detail-metadata">
        <div class="border-b border-admin-border pb-3">
          <h3 class="m-0 text-sm font-semibold text-admin-ink">Current registration</h3>
          <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">Definition metadata is read-only after creation in the current control API.</p>
        </div>
        <dl class="mt-4 grid gap-3 md:grid-cols-2 xl:grid-cols-3">
          <div class="rounded-md border border-admin-border bg-admin-panel p-3"><dt class="text-xs font-semibold uppercase text-admin-muted">Latest version</dt><dd class="m-0 mt-1 text-sm font-medium text-admin-ink" data-testid="extension-detail-version">{loadState.extension.latest_version ?? 'Not published'}</dd></div>
          <div class="rounded-md border border-admin-border bg-admin-panel p-3"><dt class="text-xs font-semibold uppercase text-admin-muted">Created</dt><dd class="m-0 mt-1 text-sm text-admin-ink">{formatDateTime(loadState.extension.created_at)}</dd></div>
          <div class="rounded-md border border-admin-border bg-admin-panel p-3"><dt class="text-xs font-semibold uppercase text-admin-muted">Updated</dt><dd class="m-0 mt-1 text-sm text-admin-ink">{formatDateTime(loadState.extension.updated_at)}</dd></div>
          <div class="rounded-md border border-admin-border bg-admin-panel p-3 md:col-span-2 xl:col-span-3"><dt class="text-xs font-semibold uppercase text-admin-muted">Labels</dt><dd class="m-0 mt-1 text-sm text-admin-ink">{labelSummary(loadState.extension.labels)}</dd></div>
        </dl>
      </section>

      <section class="rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="extension-version-form">
        <div class="border-b border-admin-border pb-3">
          <h3 class="m-0 text-sm font-semibold text-admin-ink">Publish installed version</h3>
          <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">The path must be absolute and already available inside supported docker-backed browser runtimes. BrowserPane does not upload extension packages here.</p>
        </div>
        <form class="mt-4 grid gap-4 lg:grid-cols-[220px_minmax(0,1fr)_auto] lg:items-start" onsubmit={(event) => { event.preventDefault(); publishVersion(); }}>
          <label class="grid gap-1.5 text-sm"><span class="font-medium text-admin-ink">Version</span><input class="h-10 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none focus:border-admin-accent" type="text" bind:value={versionDraft.version} disabled={busy} placeholder="1.0.0" data-testid="extension-version-value" />{#if versionValidation.fieldErrors.version}<AdminMessage tone="error" density="compact" items={versionValidation.fieldErrors.version} testId="extension-version-value-error" />{/if}</label>
          <label class="grid gap-1.5 text-sm"><span class="font-medium text-admin-ink">Installed path</span><input class="h-10 rounded-md border border-admin-border bg-white px-3 font-mono text-xs text-admin-ink outline-none focus:border-admin-accent" type="text" bind:value={versionDraft.installPath} disabled={busy} placeholder="/opt/browserpane/extensions/example" data-testid="extension-version-path" />{#if versionValidation.fieldErrors.installPath}<AdminMessage tone="error" density="compact" items={versionValidation.fieldErrors.installPath} testId="extension-version-path-error" />{/if}</label>
          <button class="mt-[26px] inline-flex h-10 items-center justify-center rounded-md border border-admin-accent bg-admin-accent px-3 text-sm font-semibold text-white disabled:opacity-60" type="submit" disabled={busy || !versionValidation.valid} data-testid="extension-version-submit">Publish version</button>
        </form>
      </section>
    </div>
  {/if}
</aside>
