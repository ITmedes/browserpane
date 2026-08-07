import { afterEach, describe, expect, it } from 'vitest';
import { apiContractEvidenceFixture } from '$lib/test-utils/api-contract-fixtures';
import { byTestId, cleanupRenderedComponents, renderComponent } from '$lib/test-utils/svelte-component-test';
import AdminDocsWorkspace from './AdminDocsWorkspace.svelte';

afterEach(cleanupRenderedComponents);

describe('AdminDocsWorkspace', () => {
  it('renders contract, credential, enforcement, and separate compatibility guidance', () => {
    const target = renderComponent(AdminDocsWorkspace, { evidence: apiContractEvidenceFixture() });
    expect(byTestId(target, 'docs-contract-boundary').textContent).toContain('Frozen v1 contract');
    expect(byTestId(target, 'docs-secret-boundary').textContent).toContain('No credential interchange');
    expect(byTestId(target, 'docs-conformance').textContent).toContain('Semantic compatibility');
    expect(byTestId(target, 'docs-compatibility-boundary').textContent).toContain('not frozen v1');
    expect(target.querySelectorAll('[data-testid="docs-compatibility-row"]')).toHaveLength(2);
    expect(byTestId(target, 'docs-openapi-download').getAttribute('href')).toBe('/openapi/bpane-control-v1.yaml');
    expect(target.textContent).toContain('/api/session/status');
    expect(target.textContent).not.toContain('access_token');
  });
});
