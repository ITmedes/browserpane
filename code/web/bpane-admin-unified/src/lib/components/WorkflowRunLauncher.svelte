<script lang="ts">
  import { ExternalLink, Play, RefreshCw } from '@lucide/svelte';
  import type {
    CreateWorkflowRunRequest,
    WorkflowRunLaunchResult,
  } from '$lib/workflow-runs/workflow-run-types';
  import {
    defaultSessionMode,
    initialWorkflowInputText,
    inputParametersFromSchema,
    validateWorkflowRunLaunch,
    workflowRunRequestPreview,
    type WorkflowInputParameter,
    type WorkflowRunSessionMode,
  } from '$lib/workflows/workflow-run-launcher-view-model';
  import type { WorkflowDefinitionVersionResource } from '$lib/workflows/workflow-types';
  import AdminMessage from './AdminMessage.svelte';

  type WorkflowRunLauncherProps = {
    readonly workflowId: string;
    readonly selectedVersion?: WorkflowDefinitionVersionResource | null;
    readonly disabled?: boolean;
    readonly onStartRun?: (
      request: CreateWorkflowRunRequest,
      options: { readonly connectPreview: boolean },
    ) => Promise<WorkflowRunLaunchResult>;
  };

  type SubmitState =
    | { readonly status: 'idle' }
    | { readonly status: 'running'; readonly label: string }
    | { readonly status: 'success'; readonly result: WorkflowRunLaunchResult }
    | { readonly status: 'warning'; readonly result: WorkflowRunLaunchResult; readonly message: string }
    | { readonly status: 'error'; readonly message: string };

  let {
    workflowId,
    selectedVersion = null,
    disabled = false,
    onStartRun,
  }: WorkflowRunLauncherProps = $props();

  let inputText = $state('{}');
  let inputValues = $state<Record<string, string>>({});
  let sessionMode = $state<WorkflowRunSessionMode>('create_session');
  let existingSessionId = $state('');
  let projectId = $state('');
  let versionKey = $state('');
  let submitState = $state<SubmitState>({ status: 'idle' });

  const inputParameters = $derived(inputParametersFromSchema(selectedVersion?.input_schema));
  const launchDisabled = $derived(disabled || !selectedVersion || submitState.status === 'running');
  const requestDraft = $derived({
    workflowId,
    version: selectedVersion?.version ?? '',
    inputSchema: selectedVersion?.input_schema,
    inputText,
    sessionMode,
    existingSessionId,
    projectId,
  });
  const payloadPreview = $derived(workflowRunRequestPreview(requestDraft));

  $effect(() => {
    const nextVersionKey = selectedVersion?.id ?? selectedVersion?.version ?? '';
    if (nextVersionKey === versionKey) {
      return;
    }
    versionKey = nextVersionKey;
    inputText = initialWorkflowInputText(selectedVersion?.input_schema);
    inputValues = fieldValuesFromInput(inputParameters, inputText);
    sessionMode = defaultSessionMode(selectedVersion);
    existingSessionId = '';
    projectId = '';
    submitState = { status: 'idle' };
  });

  function updateInputParameter(parameter: WorkflowInputParameter, value: string): void {
    inputValues = { ...inputValues, [parameter.name]: value };
    const object = parseInputObject(inputText);
    object[parameter.name] = valueForParameter(parameter, value);
    inputText = JSON.stringify(object, null, 2);
  }

  function handleRawInput(value: string): void {
    inputText = value;
  }

  async function startRun(connectPreview: boolean): Promise<void> {
    const validation = validateWorkflowRunLaunch(requestDraft);
    if (!validation.ok) {
      submitState = { status: 'error', message: validation.message };
      return;
    }
    if (!onStartRun) {
      submitState = { status: 'error', message: 'Workflow run launcher is not connected.' };
      return;
    }
    submitState = {
      status: 'running',
      label: connectPreview ? 'Starting workflow run and opening preview...' : 'Starting workflow run...',
    };
    try {
      const result = await onStartRun(validation.request, { connectPreview });
      submitState = result.previewBlocked
        ? {
            status: 'warning',
            result,
            message: 'Workflow run created, but the browser preview popup was blocked.',
          }
        : { status: 'success', result };
    } catch (error) {
      submitState = {
        status: 'error',
        message: error instanceof Error ? error.message : 'Workflow run start failed.',
      };
    }
  }

  function parseInputObject(value: string): Record<string, unknown> {
    try {
      const parsed = JSON.parse(value);
      if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
        return { ...parsed };
      }
    } catch {
      // Keep field editing usable even while the raw JSON editor is invalid.
    }
    return {};
  }

  function fieldValuesFromInput(
    parameters: readonly WorkflowInputParameter[],
    value: string,
  ): Record<string, string> {
    const object = parseInputObject(value);
    const values: Record<string, string> = {};
    for (const parameter of parameters) {
      const current = object[parameter.name] ?? parameter.defaultValue;
      values[parameter.name] = parameter.kind === 'json'
        ? JSON.stringify(current ?? {}, null, 2)
        : String(current ?? '');
    }
    return values;
  }

  function valueForParameter(parameter: WorkflowInputParameter, value: string): unknown {
    if (parameter.kind === 'number') {
      return value.trim() === '' ? 0 : Number(value);
    }
    if (parameter.kind === 'boolean') {
      return value === 'true';
    }
    if (parameter.kind === 'json') {
      try {
        return JSON.parse(value);
      } catch {
        return value;
      }
    }
    return value;
  }

  function runDetailHref(runId: string): string {
    return `/admin-new/workflow-runs/${encodeURIComponent(runId)}`;
  }

  function sessionDetailHref(sessionId: string): string {
    return `/admin-new/sessions/${encodeURIComponent(sessionId)}`;
  }
</script>

<section class="min-w-0 rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="workflow-run-launcher">
  <div class="flex flex-col gap-3 border-b border-admin-border pb-3 md:flex-row md:items-start md:justify-between">
    <div class="min-w-0">
      <h3 class="m-0 text-sm font-semibold text-admin-ink">Run workflow</h3>
      <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">
        Configure one run instance from the selected version. Runs are asynchronous and create or reuse a browser session.
      </p>
    </div>
    {#if selectedVersion}
      <span class="inline-flex w-fit rounded-full bg-admin-panel px-2 py-0.5 text-xs font-semibold text-admin-ink ring-1 ring-admin-border" data-testid="workflow-run-launch-version">
        {selectedVersion.version}
      </span>
    {/if}
  </div>

  {#if !selectedVersion}
    <div class="mt-4">
      <AdminMessage
        tone="warning"
        title="No workflow version selected"
        message="Publish or select a workflow version before starting a run."
        testId="workflow-run-launch-unavailable"
      />
    </div>
  {:else}
    <div class="mt-4 grid min-w-0 grid-cols-[minmax(0,1fr)] gap-4 xl:grid-cols-[minmax(0,1fr)_minmax(320px,0.8fr)]">
      <div class="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-4">
        <section class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-3" data-testid="workflow-run-input-parameters">
          <div class="flex flex-col gap-1 border-b border-admin-border pb-3">
            <h4 class="m-0 text-sm font-semibold text-admin-ink">Input parameters</h4>
            <p class="m-0 text-xs leading-5 text-admin-muted">
              {#if inputParameters.length === 0}
                This version does not define a flat input schema. Use the JSON payload editor below.
              {:else}
                Values are generated from the version input schema and kept in sync with the JSON payload.
              {/if}
            </p>
          </div>

          {#if inputParameters.length > 0}
            <div class="mt-3 grid min-w-0 grid-cols-[minmax(0,1fr)] gap-3 md:grid-cols-2">
              {#each inputParameters as parameter}
                <label class="grid min-w-0 gap-1 text-sm text-admin-ink" data-testid="workflow-run-input-field">
                  <span class="flex min-w-0 items-center gap-2">
                    <span class="truncate font-semibold">{parameter.label}</span>
                    {#if parameter.required}
                      <span class="rounded-full bg-amber-50 px-2 py-0.5 text-[11px] font-semibold text-amber-700 ring-1 ring-amber-200">required</span>
                    {/if}
                  </span>
                  {#if parameter.description}
                    <span class="text-xs leading-5 text-admin-muted">{parameter.description}</span>
                  {/if}
                  {#if parameter.kind === 'boolean'}
                    <select
                      class="h-10 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none focus:border-admin-accent"
                      value={inputValues[parameter.name] ?? 'false'}
                      onchange={(event) => updateInputParameter(parameter, event.currentTarget.value)}
                      data-testid={`workflow-run-input-${parameter.name}`}
                    >
                      <option value="false">false</option>
                      <option value="true">true</option>
                    </select>
                  {:else}
                    <input
                      class="h-10 min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none placeholder:text-admin-muted focus:border-admin-accent"
                      type={parameter.kind === 'number' ? 'number' : 'text'}
                      value={inputValues[parameter.name] ?? ''}
                      placeholder={parameter.placeholder}
                      oninput={(event) => updateInputParameter(parameter, event.currentTarget.value)}
                      data-testid={`workflow-run-input-${parameter.name}`}
                    />
                  {/if}
                </label>
              {/each}
            </div>
          {/if}
        </section>

        <section class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-3" data-testid="workflow-run-input-json-panel">
          <div class="flex flex-col gap-1 border-b border-admin-border pb-3">
            <h4 class="m-0 text-sm font-semibold text-admin-ink">Input JSON</h4>
            <p class="m-0 text-xs leading-5 text-admin-muted">Advanced input payload sent as the workflow run `input` value.</p>
          </div>
          <textarea
            class="mt-3 min-h-44 w-full resize-y rounded-md border border-admin-border bg-white p-3 font-mono text-xs leading-5 text-admin-ink outline-none focus:border-admin-accent"
            spellcheck="false"
            value={inputText}
            oninput={(event) => handleRawInput(event.currentTarget.value)}
            data-testid="workflow-run-input-json"
          ></textarea>
        </section>
      </div>

      <div class="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-4">
        <section class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-3" data-testid="workflow-run-session-binding">
          <div class="border-b border-admin-border pb-3">
            <h4 class="m-0 text-sm font-semibold text-admin-ink">Session binding</h4>
          </div>

          <div class="mt-3 grid min-w-0 grid-cols-[minmax(0,1fr)] gap-3">
            <label class="grid min-w-0 gap-1 text-sm text-admin-ink">
              <span class="font-semibold">Mode</span>
              <select
                class="h-10 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none focus:border-admin-accent"
                bind:value={sessionMode}
                data-testid="workflow-run-session-mode"
              >
                <option value="version_default" disabled={!selectedVersion.default_session}>Version default session</option>
                <option value="create_session">Create new session</option>
                <option value="existing_session">Use existing session</option>
              </select>
            </label>

            <label class="grid min-w-0 gap-1 text-sm text-admin-ink">
              <span class="font-semibold">Project id</span>
              <input
                class="h-10 min-w-0 rounded-md border border-admin-border bg-white px-3 font-mono text-sm text-admin-ink outline-none placeholder:text-admin-muted focus:border-admin-accent"
                type="text"
                placeholder="Optional project id"
                bind:value={projectId}
                data-testid="workflow-run-project-id"
              />
            </label>

            {#if sessionMode === 'existing_session'}
              <label class="grid min-w-0 gap-1 text-sm text-admin-ink">
                <span class="font-semibold">Existing session id</span>
                <input
                  class="h-10 min-w-0 rounded-md border border-admin-border bg-white px-3 font-mono text-sm text-admin-ink outline-none placeholder:text-admin-muted focus:border-admin-accent"
                  type="text"
                  placeholder="Session id"
                  bind:value={existingSessionId}
                  data-testid="workflow-run-existing-session-id"
                />
              </label>
            {/if}
          </div>
        </section>

        <section class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-3" data-testid="workflow-run-payload-preview-panel">
          <div class="border-b border-admin-border pb-3">
            <h4 class="m-0 text-sm font-semibold text-admin-ink">API payload</h4>
          </div>
          <pre class="mt-3 max-h-72 overflow-auto rounded-md border border-admin-border bg-white p-3 text-xs leading-5 text-admin-ink" data-testid="workflow-run-payload-preview">{payloadPreview}</pre>
        </section>
      </div>
    </div>

    {#if submitState.status === 'success' || submitState.status === 'warning'}
      <div class="mt-4" data-testid="workflow-run-launch-success">
        <AdminMessage
          tone={submitState.status === 'warning' ? 'warning' : 'success'}
          title="Workflow run created"
          message={`${submitState.result.run.id} is bound to session ${submitState.result.run.session_id}.`}
        />
        {#if submitState.status === 'warning'}
          <p class="m-0 mt-2 text-sm text-amber-700">{submitState.message}</p>
        {/if}
        <div class="mt-3 flex flex-wrap gap-2">
          <a
            class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft"
            href={runDetailHref(submitState.result.run.id)}
            data-testid="workflow-run-open-run"
          >
            <ExternalLink size={15} strokeWidth={1.8} />
            <span>Open run catalog</span>
          </a>
          <a
            class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft"
            href={sessionDetailHref(submitState.result.run.session_id)}
            data-testid="workflow-run-open-session"
          >
            <ExternalLink size={15} strokeWidth={1.8} />
            <span>Open session</span>
          </a>
        </div>
      </div>
    {:else if submitState.status === 'error'}
      <div class="mt-4">
        <AdminMessage
          tone="error"
          title="Workflow run start failed"
          message={submitState.message}
          testId="workflow-run-launch-error"
        />
      </div>
    {:else if submitState.status === 'running'}
      <div class="mt-4">
        <AdminMessage
          tone="loading"
          title={submitState.label}
          testId="workflow-run-launch-running"
        />
      </div>
    {/if}

    <div class="mt-4 flex flex-wrap gap-2">
      <button
        class="inline-flex h-10 items-center gap-2 rounded-md border border-admin-accent bg-admin-accent px-3 text-sm font-semibold text-white shadow-sm hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-60"
        type="button"
        onclick={() => void startRun(false)}
        disabled={launchDisabled}
        data-testid="workflow-run-start"
      >
        <Play size={16} strokeWidth={1.9} />
        <span>Start</span>
      </button>
      <button
        class="inline-flex h-10 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink shadow-sm hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
        type="button"
        onclick={() => void startRun(true)}
        disabled={launchDisabled}
        data-testid="workflow-run-start-connect"
      >
        <ExternalLink size={16} strokeWidth={1.9} />
        <span>Start and connect</span>
      </button>
      <button
        class="inline-flex h-10 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink shadow-sm hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
        type="button"
        onclick={() => {
          inputText = initialWorkflowInputText(selectedVersion.input_schema);
          inputValues = fieldValuesFromInput(inputParameters, inputText);
          submitState = { status: 'idle' };
        }}
        disabled={launchDisabled}
        data-testid="workflow-run-reset-input"
      >
        <RefreshCw size={16} strokeWidth={1.9} />
        <span>Reset input</span>
      </button>
    </div>
  {/if}
</section>
