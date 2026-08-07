import { afterEach, describe, expect, it } from 'vitest';
import { apiContractEvidenceFixture } from '$lib/test-utils/api-contract-fixtures';
import { byTestId, cleanupRenderedComponents, renderComponent } from '$lib/test-utils/svelte-component-test';
import ApiCompanionWorkspace from './ApiCompanionWorkspace.svelte';

afterEach(cleanupRenderedComponents);

describe('ApiCompanionWorkspace', () => {
  it('renders contract-derived task flows, links, and credential boundary', () => {
    const target = renderComponent(ApiCompanionWorkspace, { evidence: apiContractEvidenceFixture() });
    expect(byTestId(target, 'api-download-openapi').getAttribute('href')).toBe('/openapi/bpane-control-v1.yaml');
    expect(byTestId(target, 'api-open-coverage').getAttribute('href')).toBe('/admin-new/coverage');
    expect(byTestId(target, 'api-credential-boundary').textContent).toContain('never reads or inserts');
    expect(target.querySelectorAll('[data-testid^="api-task-"][data-testid$="s"]')).toBeTruthy();
    expect(byTestId(target, 'api-task-projects')).toBeTruthy();
    expect(byTestId(target, 'api-task-sessions')).toBeTruthy();
    expect(byTestId(target, 'api-task-workflows')).toBeTruthy();
    expect(byTestId(target, 'api-task-file-workspaces')).toBeTruthy();
    expect(target.textContent).not.toContain('owner-token');
    expect(target.textContent).not.toContain('client_secret');
  });
});
