import { afterEach, describe, expect, it, vi } from 'vitest';
import { apiContractEvidenceFixture } from '$lib/test-utils/api-contract-fixtures';
import { byTestId, cleanupRenderedComponents, renderComponent } from '$lib/test-utils/svelte-component-test';
import ApiCoverageWorkspace from './ApiCoverageWorkspace.svelte';

afterEach(cleanupRenderedComponents);

describe('ApiCoverageWorkspace', () => {
  it('renders exact inventory and filters without changing classifications', async () => {
    window.history.replaceState({}, '', '/admin-new/coverage');
    const target = renderComponent(ApiCoverageWorkspace, { evidence: apiContractEvidenceFixture() });
    expect(target.querySelectorAll('[data-testid="api-operation-row"]')).toHaveLength(12);
    expect(byTestId(target, 'api-coverage-integrity').textContent).toContain('12 operations');

    const classification = byTestId(target, 'api-coverage-classification') as HTMLSelectElement;
    classification.value = 'internal-worker';
    classification.dispatchEvent(new Event('change', { bubbles: true }));
    await vi.waitFor(() => expect(target.querySelectorAll('[data-testid="api-operation-row"]')).toHaveLength(1));
    expect(target.textContent).toContain('appendWorkflowRunLog');

    byTestId(target, 'api-coverage-clear').click();
    await vi.waitFor(() => expect(target.querySelectorAll('[data-testid="api-operation-row"]')).toHaveLength(12));
  });

  it('reads operation deep-link search and renders the empty state', async () => {
    window.history.replaceState({}, '', '/admin-new/coverage?operation=createProject');
    const target = renderComponent(ApiCoverageWorkspace, { evidence: apiContractEvidenceFixture() });
    await vi.waitFor(() => expect(target.querySelectorAll('[data-testid="api-operation-row"]')).toHaveLength(1));
    expect(target.textContent).toContain('createProject');

    const search = byTestId(target, 'api-coverage-search') as HTMLInputElement;
    search.value = 'not-an-operation';
    search.dispatchEvent(new Event('input', { bubbles: true }));
    await vi.waitFor(() => expect(byTestId(target, 'api-coverage-empty').textContent).toContain('No operations match'));
  });
});
