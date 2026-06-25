<script lang="ts">
  type FieldFeedbackProps = {
    readonly errors?: readonly string[];
    readonly hint?: string;
    readonly testId?: string;
  };

  let {
    errors = [],
    hint,
    testId,
  }: FieldFeedbackProps = $props();

  const hasErrors = $derived(errors.length > 0);
</script>

<div class="min-h-5 text-xs leading-5" aria-live={hasErrors ? 'assertive' : 'polite'}>
  {#if hasErrors}
    {#if errors.length === 1}
      <p class="m-0 text-red-700" data-testid={testId}>{errors[0]}</p>
    {:else}
      <ul class="m-0 list-disc pl-4 text-red-700" data-testid={testId}>
        {#each errors as error}
          <li>{error}</li>
        {/each}
      </ul>
    {/if}
  {:else if hint}
    <p class="m-0 text-admin-muted">{hint}</p>
  {:else}
    <span class="block" aria-hidden="true">&nbsp;</span>
  {/if}
</div>
