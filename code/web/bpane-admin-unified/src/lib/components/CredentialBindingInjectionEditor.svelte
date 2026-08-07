<script lang="ts">
  import {
    CREDENTIAL_INJECTION_MODE_OPTIONS,
    type CredentialBindingDraft,
    type CredentialBindingValidation,
  } from '$lib/credential-bindings/credential-binding-form-model';
  import AdminMessage from './AdminMessage.svelte';

  let {
    draft = $bindable(),
    validation,
    disabled = false,
  }: {
    draft: CredentialBindingDraft;
    readonly validation: CredentialBindingValidation;
    readonly disabled?: boolean;
  } = $props();
</script>

<section class="grid gap-4 rounded-md border border-admin-border bg-admin-soft/50 p-4">
  <div>
    <h3 class="m-0 text-sm font-semibold text-admin-ink">Injection policy</h3>
    <p class="m-0 mt-1 text-xs text-admin-muted">
      Origins constrain where the worker may resolve and apply this credential.
    </p>
  </div>
  <label class="grid gap-1.5 text-sm"
    ><span class="font-medium text-admin-ink">Injection mode</span><select
      class="h-10 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink"
      bind:value={draft.injectionMode}
      {disabled}
      data-testid="credential-binding-injection-mode"
      >{#each CREDENTIAL_INJECTION_MODE_OPTIONS as option}<option value={option.value}
          >{option.label}</option
        >{/each}</select
    ><span class="text-xs text-admin-muted"
      >{CREDENTIAL_INJECTION_MODE_OPTIONS.find((option) => option.value === draft.injectionMode)
        ?.description}</span
    ></label
  >
  <label class="grid gap-1.5 text-sm"
    ><span class="font-medium text-admin-ink">Allowed origins</span><textarea
      class="min-h-20 rounded-md border border-admin-border bg-white px-3 py-2 font-mono text-xs text-admin-ink"
      bind:value={draft.allowedOriginsText}
      {disabled}
      placeholder="https://portal.example"
      data-testid="credential-binding-origins"
    ></textarea><span class="text-xs text-admin-muted"
      >One exact HTTP or HTTPS origin per line.</span
    >{#if validation.fieldErrors.allowedOrigins}<AdminMessage
        tone="error"
        density="compact"
        items={validation.fieldErrors.allowedOrigins}
        testId="credential-binding-origins-error"
      />{/if}</label
  >
  {#if draft.injectionMode === 'totp_fill'}
    <div class="grid gap-4 md:grid-cols-2" data-testid="credential-binding-totp-fields">
      <label class="grid gap-1.5 text-sm"
        ><span class="font-medium text-admin-ink">Issuer</span><input
          class="h-10 rounded-md border border-admin-border bg-white px-3 text-sm"
          bind:value={draft.totpIssuer}
          {disabled}
          data-testid="credential-binding-totp-issuer"
        /></label
      ><label class="grid gap-1.5 text-sm"
        ><span class="font-medium text-admin-ink">Account name</span><input
          class="h-10 rounded-md border border-admin-border bg-white px-3 text-sm"
          bind:value={draft.totpAccountName}
          {disabled}
          data-testid="credential-binding-totp-account"
        /></label
      >
      {#each [{ key: 'totpPeriodSec', label: 'Period seconds', testId: 'period' }, { key: 'totpDigits', label: 'Digits', testId: 'digits' }] as field}<label
          class="grid gap-1.5 text-sm"
          ><span class="font-medium text-admin-ink">{field.label}</span><input
            class="h-10 rounded-md border border-admin-border bg-white px-3 text-sm"
            type="number"
            min="1"
            bind:value={draft[field.key as 'totpPeriodSec' | 'totpDigits']}
            {disabled}
            data-testid={`credential-binding-totp-${field.testId}`}
          />{#if validation.fieldErrors[field.key]}<AdminMessage
              tone="error"
              density="compact"
              items={validation.fieldErrors[field.key]}
              testId={`credential-binding-totp-${field.testId}-error`}
            />{/if}</label
        >{/each}
    </div>
  {/if}
</section>
