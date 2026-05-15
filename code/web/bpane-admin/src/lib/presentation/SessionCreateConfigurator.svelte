<script lang="ts">
  import { base } from '$app/paths';
  import type { CreateSessionCommand } from '../api/control-types';
  import {
    SESSION_CREATE_OWNER_MODES,
    defaultSessionCreateFormState,
    validateSessionCreateForm,
  } from './session-create-configurator';
  import AdminMessage from './AdminMessage.svelte';

  type SessionCreateConfiguratorProps = {
    readonly onCreateSession: (command: CreateSessionCommand) => void;
    readonly loading?: boolean;
    readonly disabled?: boolean;
    readonly submitTestId?: string;
    readonly submitLabel?: string;
    readonly variant?: 'panel' | 'inline';
    readonly showFileWorkspaceLink?: boolean;
    readonly payloadInitiallyOpen?: boolean;
    readonly payloadOpen?: boolean;
    readonly onPayloadOpenChange?: (open: boolean) => void;
  };

  let {
    onCreateSession,
    loading = false,
    disabled = false,
    submitTestId = 'session-new',
    submitLabel = 'Create session',
    variant = 'inline',
    showFileWorkspaceLink = true,
    payloadInitiallyOpen = false,
    payloadOpen: controlledPayloadOpen,
    onPayloadOpenChange,
  }: SessionCreateConfiguratorProps = $props();

  const defaults = defaultSessionCreateFormState();
  let ownerMode = $state(defaults.ownerMode);
  let idleTimeoutSec = $state(defaults.idleTimeoutSec);
  let labels = $state(defaults.labels);
  let payloadOpenInternal = $state(false);
  const validation = $derived(validateSessionCreateForm({ ownerMode, idleTimeoutSec, labels }));
  const rootClass = $derived(variant === 'panel'
    ? 'admin-panel mt-0 grid gap-4'
    : 'grid gap-3 rounded-[16px] border border-admin-ink/10 bg-admin-panel/62 p-3');
  const payloadOpen = $derived(controlledPayloadOpen ?? payloadOpenInternal);

  $effect(() => {
    if (payloadInitiallyOpen) {
      payloadOpenInternal = true;
    }
  });

  function submit(): void {
    if (!validation.command) {
      return;
    }
    onCreateSession(validation.command);
  }

  function setPayloadOpen(open: boolean): void {
    payloadOpenInternal = open;
    onPayloadOpenChange?.(open);
  }
</script>

<section class={rootClass} aria-label="Create session" data-testid="session-create-configurator">
  <div class="flex min-w-0 flex-wrap items-start justify-between gap-3">
    <div class="min-w-0">
      <p class="admin-eyebrow mb-1">Create session</p>
      <h3 class="m-0 text-base font-bold text-admin-ink">Session configuration</h3>
    </div>
    {#if showFileWorkspaceLink}
      <a class="admin-button-ghost min-h-9 px-3 text-sm" href={`${base}/files/workspaces`}>
        File workspaces
      </a>
    {/if}
  </div>

  <form
    class="grid gap-3"
    onsubmit={(event) => {
      event.preventDefault();
      submit();
    }}
  >
    <div class="grid gap-3 lg:grid-cols-[minmax(180px,1fr)_minmax(180px,1fr)]">
      <label class="grid gap-1 text-sm font-bold text-admin-ink/72">
        Owner mode
        <select
          class="min-h-11 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
          data-testid="session-create-owner-mode"
          bind:value={ownerMode}
          disabled={loading || disabled}
        >
          {#each SESSION_CREATE_OWNER_MODES as mode}
            <option value={mode.value}>{mode.label}</option>
          {/each}
        </select>
      </label>

      <label class="grid gap-1 text-sm font-bold text-admin-ink/72">
        Idle timeout
        <input
          class="min-h-11 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
          data-testid="session-create-idle-timeout"
          inputmode="numeric"
          placeholder="Backend default"
          type="text"
          bind:value={idleTimeoutSec}
          disabled={loading || disabled}
        />
      </label>
    </div>

    <label class="grid gap-1 text-sm font-bold text-admin-ink/72">
      Labels
      <textarea
        class="min-h-20 resize-y rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 py-2 text-admin-ink outline-none focus:border-admin-leaf/45"
        data-testid="session-create-labels"
        placeholder="case=1234, purpose=import-repro"
        rows={variant === 'panel' ? 3 : 2}
        bind:value={labels}
        disabled={loading || disabled}
      ></textarea>
    </label>

    <section class="rounded-xl border border-admin-ink/10 bg-admin-field/68 p-3" aria-label="API payload preview">
      <button
        class="flex w-full cursor-pointer items-center justify-between gap-3 border-0 bg-transparent p-0 text-left text-xs font-bold uppercase text-[#c1d0e8]"
        type="button"
        aria-expanded={payloadOpen}
        data-testid="session-create-preview-toggle"
        onclick={() => setPayloadOpen(!payloadOpen)}
      >
        <span>API payload</span>
        <span aria-hidden="true">{payloadOpen ? 'Hide' : 'Show'}</span>
      </button>
      {#if payloadOpen}
        <pre class="mt-3 max-h-44 overflow-auto whitespace-pre-wrap text-xs text-admin-ink" data-testid="session-create-preview">{validation.preview}</pre>
      {/if}
    </section>

    {#if validation.errors.length > 0}
      <AdminMessage
        variant="error"
        title="Invalid session configuration"
        message={validation.errors.join(' ')}
        testId="session-create-error"
        compact={true}
      />
    {/if}

    <button
      class="admin-button-primary w-fit"
      type="submit"
      data-testid={submitTestId}
      disabled={loading || disabled || validation.errors.length > 0}
    >
      {loading ? 'Creating...' : submitLabel}
    </button>
  </form>
</section>
