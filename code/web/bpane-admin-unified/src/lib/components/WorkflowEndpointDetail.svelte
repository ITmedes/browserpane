<script lang="ts">
  import { ArrowLeft, RefreshCw, ShieldCheck, ShieldX } from '@lucide/svelte';
  import type { AdminActionState } from '$lib/application/admin-async-state';
  import type {
    UpsertWorkflowEndpointGrantRequest,
    WorkflowEndpointDetailLoadState,
    WorkflowEndpointGrantOperation,
  } from '$lib/workflow-endpoints/workflow-endpoint-types';
  import {
    buildWorkflowEndpointInvocationExample,
    createWorkflowEndpointGrantDraft,
    validateWorkflowEndpointGrantDraft,
  } from '$lib/workflow-endpoints/workflow-endpoint-view-model';
  import ActionFeedback from './ActionFeedback.svelte';
  import AdminMessage from './AdminMessage.svelte';
  import FieldFeedback from './FieldFeedback.svelte';

  let {
    state: loadState,
    actionState = { status: 'idle' },
    onRefresh,
    onActivate,
    onDisable,
    onGrant,
    onRevoke,
  }: {
    readonly state: WorkflowEndpointDetailLoadState;
    readonly actionState?: AdminActionState;
    readonly onRefresh?: () => void | Promise<void>;
    readonly onActivate?: () => void | Promise<void>;
    readonly onDisable?: () => void | Promise<void>;
    readonly onGrant?: (request: UpsertWorkflowEndpointGrantRequest) => void | Promise<void>;
    readonly onRevoke?: (grantId: string) => void | Promise<void>;
  } = $props();

  let grantDraft = $state(createWorkflowEndpointGrantDraft());
  let grantErrors = $state<Readonly<Record<string, readonly string[]>>>({});
  const busy = $derived(actionState.status === 'running');
  const allOperations: readonly WorkflowEndpointGrantOperation[] = [
    'invoke',
    'read',
    'cancel',
    'artifact.read',
  ];

  function submitGrant(event: SubmitEvent): void {
    event.preventDefault();
    const validation = validateWorkflowEndpointGrantDraft(grantDraft);
    grantErrors = validation.fieldErrors;
    if (validation.request) void onGrant?.(validation.request);
  }

  function toggleOperation(operation: WorkflowEndpointGrantOperation, checked: boolean): void {
    grantDraft.operations = checked
      ? [...new Set([...grantDraft.operations, operation])]
      : grantDraft.operations.filter((candidate) => candidate !== operation);
  }
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1180px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="workflow-endpoint-detail">
  <header class="border-b border-admin-border pb-4">
    <a class="inline-flex items-center gap-2 text-sm font-medium text-admin-muted hover:text-admin-ink" href="/admin-new/workflow-endpoints"><ArrowLeft size={16} /><span>Workflow endpoints</span></a>
    <p class="m-0 mt-3 text-xs font-semibold uppercase text-admin-muted">Operate</p>
    <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">Workflow endpoint details</h1>
  </header>

  {#if loadState.status === 'idle' || loadState.status === 'loading'}
    <AdminMessage tone="loading" title="Loading workflow endpoint" message="Endpoint configuration and caller grants are being refreshed." testId="workflow-endpoint-detail-loading" />
  {:else if loadState.status === 'error'}
    <AdminMessage tone="error" title="Workflow endpoint unavailable" message={loadState.message} testId="workflow-endpoint-detail-error" />
  {:else}
    <section class="rounded-md border border-admin-border bg-admin-panel">
      <div class="flex flex-col gap-3 border-b border-admin-border p-4 lg:flex-row lg:items-start lg:justify-between">
        <div>
          <div class="flex flex-wrap items-center gap-2">
            <h2 class="m-0 font-mono text-xl font-semibold text-admin-ink" data-testid="workflow-endpoint-detail-key">{loadState.endpoint.endpoint_key}</h2>
            <span class="rounded-full bg-admin-soft px-2 py-1 text-xs font-semibold capitalize text-admin-ink" data-testid="workflow-endpoint-detail-state">{loadState.endpoint.state}</span>
          </div>
          <p class="m-0 mt-2 max-w-3xl text-sm text-admin-muted">{loadState.endpoint.purpose}</p>
          <p class="m-0 mt-2 font-mono text-xs text-admin-muted">Project {loadState.endpoint.project_id}</p>
        </div>
        <div class="flex flex-wrap gap-2">
          <button class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm disabled:opacity-60" type="button" disabled={busy} onclick={() => void onRefresh?.()} data-testid="workflow-endpoint-detail-refresh"><RefreshCw size={15} />Refresh</button>
          {#if loadState.endpoint.state !== 'active'}
            <button class="inline-flex h-9 items-center gap-2 rounded-md bg-admin-accent px-3 text-sm font-semibold text-white disabled:opacity-60" type="button" disabled={busy} onclick={() => void onActivate?.()} data-testid="workflow-endpoint-activate"><ShieldCheck size={15} />Activate</button>
          {:else}
            <button class="inline-flex h-9 items-center gap-2 rounded-md border border-amber-300 bg-amber-50 px-3 text-sm font-semibold text-amber-900 disabled:opacity-60" type="button" disabled={busy} onclick={() => void onDisable?.()} data-testid="workflow-endpoint-disable"><ShieldX size={15} />Disable</button>
          {/if}
        </div>
      </div>

      <div class="border-b border-admin-border p-4">
        <ActionFeedback
          state={actionState}
          successTitle="Workflow endpoint action completed"
          errorTitle="Workflow endpoint action failed"
          successTestId="workflow-endpoint-action-success"
          errorTestId="workflow-endpoint-action-error"
          runningTestId="workflow-endpoint-action-running"
          reserveSpace={false}
        />
      </div>

      <div class="grid gap-4 p-4 lg:grid-cols-2">
        <section class="rounded-md border border-admin-border p-4" data-testid="workflow-endpoint-contract">
          <h3 class="m-0 text-sm font-semibold text-admin-ink">Published contract</h3>
          <dl class="mt-3 grid gap-3 sm:grid-cols-2">
            <div><dt class="text-xs uppercase text-admin-muted">Workflow version</dt><dd class="m-0 mt-1 font-mono text-xs">{loadState.endpoint.workflow_version}</dd></div>
            <div><dt class="text-xs uppercase text-admin-muted">Controls</dt><dd class="m-0 mt-1 text-sm">{loadState.endpoint.supported_controls.join(', ')}</dd></div>
            <div><dt class="text-xs uppercase text-admin-muted">Execution timeout</dt><dd class="m-0 mt-1 text-sm">{loadState.endpoint.execution_timeout_seconds} seconds</dd></div>
            <div><dt class="text-xs uppercase text-admin-muted">Inline result limit</dt><dd class="m-0 mt-1 text-sm">{loadState.endpoint.inline_result_max_bytes} bytes</dd></div>
            <div><dt class="text-xs uppercase text-admin-muted">Artifact mode</dt><dd class="m-0 mt-1 text-sm">Authorized references</dd></div>
            <div><dt class="text-xs uppercase text-admin-muted">Artifact retention</dt><dd class="m-0 mt-1 text-sm">{loadState.endpoint.artifact_behavior.retention_seconds} seconds</dd></div>
          </dl>
          <details class="mt-4"><summary class="cursor-pointer text-sm font-semibold">Input schema</summary><pre class="mt-2 max-h-80 overflow-auto rounded bg-admin-soft p-3 text-xs">{JSON.stringify(loadState.endpoint.input_schema, null, 2)}</pre></details>
          <details class="mt-3"><summary class="cursor-pointer text-sm font-semibold">Output schema</summary><pre class="mt-2 max-h-80 overflow-auto rounded bg-admin-soft p-3 text-xs">{JSON.stringify(loadState.endpoint.output_schema, null, 2)}</pre></details>
        </section>

        <section class="rounded-md border border-admin-border p-4" data-testid="workflow-endpoint-invocation-example">
          <h3 class="m-0 text-sm font-semibold text-admin-ink">Machine invocation example</h3>
          <p class="m-0 mt-2 text-xs text-admin-muted">Use a confidential caller token and a process-stable idempotency key. BrowserPane stores neither client secret on this endpoint.</p>
          <pre class="mt-3 max-h-[28rem] overflow-auto whitespace-pre-wrap rounded bg-slate-950 p-3 text-xs text-slate-100">{buildWorkflowEndpointInvocationExample(loadState.endpoint)}</pre>
          <a class="mt-3 inline-block text-sm font-semibold text-admin-accent hover:underline" href={`/admin-new/workflow-runs?endpoint_id=${loadState.endpoint.id}`}>Inspect related workflow runs</a>
        </section>
      </div>
    </section>

    <section class="rounded-md border border-admin-border bg-admin-panel p-4" data-testid="workflow-endpoint-grants">
      <div>
        <h2 class="m-0 text-base font-semibold text-admin-ink">Narrow machine caller grants</h2>
        <p class="m-0 mt-1 text-sm text-admin-muted">These grants apply only to this project endpoint. They are not general project RBAC.</p>
      </div>
      {#if loadState.grants.length === 0}
        <p class="mt-4 rounded border border-dashed border-admin-border p-4 text-sm text-admin-muted" data-testid="workflow-endpoint-grants-empty">No service principal can call this endpoint.</p>
      {:else}
        <div class="mt-4 overflow-x-auto"><table class="w-full text-left text-sm"><thead class="text-xs uppercase text-admin-muted"><tr><th class="p-2">Service principal</th><th class="p-2">Operations</th><th class="p-2">Action</th></tr></thead><tbody>{#each loadState.grants as grant}<tr class="border-t border-admin-border" data-testid="workflow-endpoint-grant-row"><td class="p-2 font-mono text-xs">{grant.service_principal_id}</td><td class="p-2">{grant.operations.join(', ')}</td><td class="p-2"><button class="rounded border border-rose-200 bg-rose-50 px-2 py-1 text-xs font-semibold text-rose-800 disabled:opacity-60" type="button" disabled={busy} onclick={() => void onRevoke?.(grant.id)}>Revoke</button></td></tr>{/each}</tbody></table></div>
      {/if}
      <form class="mt-4 grid gap-3 border-t border-admin-border pt-4" onsubmit={submitGrant} data-testid="workflow-endpoint-grant-form">
        <label class="text-sm font-medium text-admin-ink">Registered service principal id
          <input class="mt-1 h-10 w-full rounded-md border border-admin-border px-3 font-mono text-sm" bind:value={grantDraft.servicePrincipalId} data-testid="workflow-endpoint-grant-principal" />
          <FieldFeedback errors={grantErrors.servicePrincipalId} />
        </label>
        <fieldset class="grid gap-2"><legend class="text-sm font-medium text-admin-ink">Allowed operations</legend><div class="flex flex-wrap gap-4">{#each allOperations as operation}<label class="flex items-center gap-2 text-sm"><input type="checkbox" checked={grantDraft.operations.includes(operation)} onchange={(event) => toggleOperation(operation, event.currentTarget.checked)} />{operation}</label>{/each}</div><FieldFeedback errors={grantErrors.operations} /></fieldset>
        <div class="flex justify-end"><button class="h-10 rounded-md bg-admin-accent px-4 text-sm font-semibold text-white disabled:opacity-60" type="submit" disabled={busy} data-testid="workflow-endpoint-grant-submit">Save grant</button></div>
      </form>
    </section>
  {/if}
</div>
