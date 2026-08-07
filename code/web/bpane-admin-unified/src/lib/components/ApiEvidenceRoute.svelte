<script lang="ts">
  import { onMount } from 'svelte';
  import type { Snippet } from 'svelte';
  import { ApiContractClient } from '$lib/api-companion/api-contract-client';
  import type { ApiContractEvidence, ApiContractLoadState } from '$lib/api-companion/api-contract-types';
  import AdminMessage from './AdminMessage.svelte';

  type ApiEvidenceRouteProps = {
    readonly children: Snippet<[ApiContractEvidence]>;
    readonly loadEvidence?: () => Promise<ApiContractEvidence>;
  };

  let { children, loadEvidence = defaultLoadEvidence }: ApiEvidenceRouteProps = $props();
  let state = $state<ApiContractLoadState>({ status: 'loading' });

  onMount(() => {
    void load();
  });

  async function defaultLoadEvidence(): Promise<ApiContractEvidence> {
    return await new ApiContractClient({ baseUrl: window.location.origin }).load();
  }

  async function load(): Promise<void> {
    state = { status: 'loading' };
    try {
      state = { status: 'ready', evidence: await loadEvidence() };
    } catch (error) {
      state = {
        status: 'error',
        message: error instanceof Error ? error.message : 'Unexpected API contract evidence error.',
      };
    }
  }
</script>

{#if state.status === 'ready'}
  {@render children(state.evidence)}
{:else}
  <div class="mx-auto flex min-h-full w-full max-w-[1080px] items-center px-4 py-8 sm:px-6" data-testid="api-evidence-route-state">
    <section class="w-full rounded-md border border-admin-border bg-admin-panel p-5 shadow-sm">
      {#if state.status === 'loading'}
        <AdminMessage
          tone="loading"
          title="Loading API contract evidence"
          message="The committed operation inventory, classifications, examples, and compatibility boundaries are being verified."
          testId="api-evidence-loading"
        />
      {:else}
        <AdminMessage
          tone="error"
          title="API contract evidence unavailable"
          message={state.message}
          testId="api-evidence-error"
        />
        <button
          type="button"
          class="mt-4 inline-flex h-9 items-center rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-semibold text-admin-ink hover:bg-admin-soft focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-admin-accent"
          data-testid="api-evidence-retry"
          onclick={() => void load()}
        >
          Retry
        </button>
      {/if}
    </section>
  </div>
{/if}
