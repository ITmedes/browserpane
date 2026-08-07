import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { identityAccessReviewFixture } from '$lib/test-utils/identity-fixtures';
import { byTestId, cleanupRenderedComponents, renderComponent } from '$lib/test-utils/svelte-component-test';
import IdentityMappingEditor from './IdentityMappingEditor.svelte';

afterEach(cleanupRenderedComponents);

describe('IdentityMappingEditor', () => {
  it('creates a conditional allowlisted claim mapping', async () => {
    const review = identityAccessReviewFixture();
    const onSave = vi.fn();
    const target = renderComponent(IdentityMappingEditor, {
      principal: review.principal,
      projects: review.projects,
      servicePrincipals: review.service_principals,
      onSave,
    });

    setSelect(target, 'identity-mapping-kind', 'claim');
    await tick();
    expect((byTestId(target, 'identity-mapping-save') as HTMLButtonElement).disabled).toBe(true);
    expect(byTestId(target, 'identity-mapping-claim-name-error').textContent).toContain('required');

    setInput(target, 'identity-mapping-claim-name', 'department');
    setInput(target, 'identity-mapping-external-id', 'operations');
    setInput(target, 'identity-mapping-labels', 'source=keycloak');
    await tick();
    byTestId(target, 'identity-mapping-save').click();

    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({
      kind: 'claim',
      claim_name: 'department',
      external_id: 'operations',
      project_id: 'project-1',
      labels: { source: 'keycloak' },
      service_principal_id: null,
    }));
  });

  it('derives immutable issuer and client metadata from a registered principal selection', async () => {
    const review = identityAccessReviewFixture();
    const onSave = vi.fn();
    const target = renderComponent(IdentityMappingEditor, {
      principal: review.principal,
      projects: review.projects,
      servicePrincipals: review.service_principals,
      onSave,
    });

    setInput(target, 'identity-mapping-name', '');
    setSelect(target, 'identity-mapping-kind', 'service_principal');
    await tick();

    expect((byTestId(target, 'identity-mapping-service-principal-id') as HTMLSelectElement).value).toBe('principal-1');
    expect((byTestId(target, 'identity-mapping-issuer') as HTMLInputElement).disabled).toBe(true);
    expect((byTestId(target, 'identity-mapping-external-id') as HTMLInputElement).value).toBe('bpane-mcp-bridge');
    byTestId(target, 'identity-mapping-save').click();
    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({
      name: 'MCP bridge project access',
      service_principal_id: 'principal-1',
      issuer: review.service_principals[0]?.issuer,
    }));
  });

  it('loads and updates an existing mapping', async () => {
    const review = identityAccessReviewFixture();
    const onSave = vi.fn();
    const target = renderComponent(IdentityMappingEditor, {
      principal: review.principal,
      projects: review.projects,
      servicePrincipals: review.service_principals,
      resource: review.identity_mappings[0],
      onSave,
    });

    expect((byTestId(target, 'identity-mapping-name') as HTMLInputElement).value).toBe('Demo operator access');
    setSelect(target, 'identity-mapping-state', 'disabled');
    await tick();
    byTestId(target, 'identity-mapping-save').click();
    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({ state: 'disabled', kind: 'user' }));
  });
});

function setInput(target: ParentNode, testId: string, value: string): void {
  const element = byTestId(target, testId) as HTMLInputElement | HTMLTextAreaElement;
  element.value = value;
  element.dispatchEvent(new Event('input', { bubbles: true }));
}

function setSelect(target: ParentNode, testId: string, value: string): void {
  const element = byTestId(target, testId) as HTMLSelectElement;
  element.value = value;
  element.dispatchEvent(new Event('change', { bubbles: true }));
}
