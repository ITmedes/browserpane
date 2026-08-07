<script lang="ts">
  import { ArrowUpRight, Download, FileCheck2, KeyRound, Network, ShieldCheck } from '@lucide/svelte';
  import type { ApiContractEvidence } from '$lib/api-companion/api-contract-types';
  import { AUTH_DEFINITIONS, groupCompatibilitySurfaces } from '$lib/api-companion/api-companion-view-model';
  import AdminMessage from './AdminMessage.svelte';
  import ApiContractSummary from './ApiContractSummary.svelte';

  type AdminDocsWorkspaceProps = {
    readonly evidence: ApiContractEvidence;
  };

  let { evidence }: AdminDocsWorkspaceProps = $props();
  const compatibilityGroups = $derived(groupCompatibilitySurfaces(evidence.compatibilitySurfaces));
</script>

<div class="mx-auto grid min-w-0 w-full max-w-[1440px] gap-6 px-4 py-6 sm:px-6" data-testid="admin-docs-workspace">
  <header class="flex min-w-0 flex-col gap-4 xl:flex-row xl:items-end xl:justify-between">
    <div class="min-w-0 max-w-4xl">
      <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Docs / Integration guide</p>
      <h1 class="m-0 mt-1 text-2xl font-bold text-admin-ink">Control-plane integration guide</h1>
      <p class="m-0 mt-2 text-sm leading-6 text-admin-muted">
        Contract scope, credential boundaries, compatibility surfaces, and repository enforcement for BrowserPane integrations.
      </p>
    </div>
    <div class="flex flex-wrap gap-2">
      <a class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-semibold text-admin-ink hover:bg-admin-soft" href="/admin-new/api" data-testid="docs-api-link">
        API companion
        <ArrowUpRight size={15} strokeWidth={2} />
      </a>
      <a class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-semibold text-admin-ink hover:bg-admin-soft" href="/openapi/bpane-control-v1.yaml" download data-testid="docs-openapi-download">
        <Download size={16} strokeWidth={2} />
        OpenAPI YAML
      </a>
    </div>
  </header>

  <ApiContractSummary operations={evidence.operations} compact />

  <section class="grid min-w-0 gap-4 lg:grid-cols-2" aria-label="Contract and security boundaries">
    <article class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-5 shadow-sm" data-testid="docs-contract-boundary">
      <div class="flex items-start gap-3">
        <span class="inline-flex size-9 shrink-0 items-center justify-center rounded-md bg-admin-soft text-admin-accent"><FileCheck2 size={18} strokeWidth={2} /></span>
        <div class="min-w-0">
          <h2 class="m-0 text-base font-semibold text-admin-ink">Frozen v1 contract</h2>
          <p class="m-0 mt-1 text-sm leading-6 text-admin-muted">
            `/api/v1` owner and worker operations are frozen through `bpane-control-v1.yaml`. Additive and breaking changes are checked against the supported baseline before merge.
          </p>
        </div>
      </div>
    </article>
    <article class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-5 shadow-sm" data-testid="docs-secret-boundary">
      <div class="flex items-start gap-3">
        <span class="inline-flex size-9 shrink-0 items-center justify-center rounded-md bg-admin-soft text-admin-accent"><ShieldCheck size={18} strokeWidth={2} /></span>
        <div class="min-w-0">
          <h2 class="m-0 text-base font-semibold text-admin-ink">No credential interchange</h2>
          <p class="m-0 mt-1 text-sm leading-6 text-admin-muted">
            Owner OIDC access, browser connect tickets, session automation, internal bridge credentials, and target-system credential bindings have different purposes and lifetimes.
          </p>
        </div>
      </div>
    </article>
  </section>

  <section class="min-w-0 rounded-md border border-admin-border bg-admin-panel shadow-sm" data-testid="docs-auth-domains">
    <header class="border-b border-admin-border p-4">
      <div class="flex items-center gap-2">
        <KeyRound size={17} strokeWidth={2} class="text-admin-accent" />
        <h2 class="m-0 text-base font-semibold text-admin-ink">Authentication domains</h2>
      </div>
    </header>
    <div class="grid min-w-0 gap-px bg-admin-border md:grid-cols-3">
      {#each AUTH_DEFINITIONS as definition (definition.id)}
        <div class="min-w-0 bg-admin-panel p-4">
          <h3 class="m-0 text-sm font-semibold text-admin-ink">{definition.label}</h3>
          <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">{definition.description}</p>
        </div>
      {/each}
    </div>
  </section>

  <section class="grid min-w-0 gap-4 lg:grid-cols-2" aria-label="API behavior">
    <article class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-5 shadow-sm" data-testid="docs-request-conventions">
      <h2 class="m-0 text-base font-semibold text-admin-ink">Request conventions</h2>
      <ul class="m-0 mt-3 grid gap-2 pl-5 text-sm leading-6 text-admin-muted">
        <li>Use the same-origin gateway proxy or the configured direct gateway base URL.</li>
        <li>Send JSON only where the operation declares `application/json`; uploads and downloads retain their declared binary media type.</li>
        <li>Use `client_request_id` or the operation-specific idempotency contract for retried workflow invocation.</li>
        <li>Keep project, session, workflow, and workspace identifiers explicit in integration state.</li>
      </ul>
    </article>
    <article class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-5 shadow-sm" data-testid="docs-error-conventions">
      <h2 class="m-0 text-base font-semibold text-admin-ink">Error and recovery conventions</h2>
      <ul class="m-0 mt-3 grid gap-2 pl-5 text-sm leading-6 text-admin-muted">
        <li>`400` means the request needs correction; preserve field and validation details.</li>
        <li>`401` requires a fresh credential from the correct credential domain.</li>
        <li>`404` and `410` distinguish unavailable resources from expired/retained evidence.</li>
        <li>`409` is a state or policy conflict; use `category`, `code`, and `recovery_hint` when present.</li>
        <li>`503` represents a dependency or control-plane availability failure and should be retried with bounded backoff.</li>
      </ul>
    </article>
  </section>

  <section class="min-w-0 rounded-md border border-admin-border bg-admin-panel shadow-sm" data-testid="docs-conformance">
    <header class="border-b border-admin-border p-4">
      <h2 class="m-0 text-base font-semibold text-admin-ink">Repository enforcement</h2>
      <p class="m-0 mt-1 text-sm text-admin-muted">Configured merge checks, not a claim about the current page load.</p>
    </header>
    <div class="grid min-w-0 gap-px bg-admin-border sm:grid-cols-2 xl:grid-cols-5">
      {#each ['Redocly lint', 'Schema examples', 'Axum route recognition', 'Semantic compatibility', 'Classification coverage'] as check}
        <div class="flex min-w-0 items-center gap-2 bg-admin-panel p-4 text-sm font-semibold text-admin-ink">
          <FileCheck2 size={16} strokeWidth={2} class="shrink-0 text-emerald-700" />
          <span>{check}</span>
        </div>
      {/each}
    </div>
  </section>

  <AdminMessage
    tone="warning"
    title="Compatibility is not frozen v1"
    message="The surfaces below exist for deployment, identity-provider, MCP protocol, local trust, or migration needs. They follow their own lifecycle and must not be inferred to have the frozen owner API compatibility guarantee."
    testId="docs-compatibility-boundary"
  />

  <section class="grid min-w-0 gap-4" aria-label="Compatibility surfaces" data-testid="docs-compatibility-surfaces">
    {#each compatibilityGroups as group (group.family)}
      <article class="min-w-0 overflow-hidden rounded-md border border-admin-border bg-admin-panel shadow-sm">
        <header class="flex items-center gap-2 border-b border-admin-border bg-admin-soft px-4 py-3">
          <Network size={16} strokeWidth={2} class="text-admin-accent" />
          <h2 class="m-0 text-sm font-semibold text-admin-ink">{group.family}</h2>
        </header>
        <div class="divide-y divide-admin-border">
          {#each group.surfaces as surface (surface.id)}
            <div class="grid min-w-0 gap-2 p-4 lg:grid-cols-[minmax(0,1fr)_minmax(16rem,2fr)]" data-testid="docs-compatibility-row">
              <div class="min-w-0">
                <div class="flex min-w-0 flex-wrap items-center gap-2 text-xs">
                  <span class="rounded-full bg-admin-soft px-2 py-0.5 font-bold text-admin-ink">{surface.methods.join(' / ')}</span>
                  <span class="rounded-full bg-admin-soft px-2 py-0.5 font-semibold text-admin-muted">{surface.stability}</span>
                  <span class="rounded-full bg-admin-soft px-2 py-0.5 font-semibold text-admin-muted">{surface.auth}</span>
                </div>
                <code class="mt-2 block break-all text-xs text-admin-ink">{surface.path}</code>
              </div>
              <p class="m-0 text-sm leading-6 text-admin-muted">{surface.purpose}</p>
            </div>
          {/each}
        </div>
      </article>
    {/each}
  </section>

  <section class="grid min-w-0 gap-3 sm:grid-cols-2 xl:grid-cols-4" aria-label="Contract downloads" data-testid="docs-contract-downloads">
    {#each [
      ['/openapi/bpane-control-v1.yaml', 'OpenAPI YAML'],
      ['/openapi/bpane-control-v1.operations.json', 'Operation inventory'],
      ['/openapi/bpane-control-v1.classifications.json', 'Classifications'],
      ['/openapi/bpane-control-v1.examples.json', 'Validated examples'],
    ] as download}
      <a class="inline-flex min-h-12 items-center justify-between gap-3 rounded-md border border-admin-border bg-admin-panel px-4 text-sm font-semibold text-admin-ink shadow-sm hover:bg-admin-soft" href={download[0]} download>
        {download[1]}
        <Download size={16} strokeWidth={2} />
      </a>
    {/each}
  </section>
</div>
