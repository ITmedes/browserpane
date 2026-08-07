import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { identityAccessReviewFixture } from '$lib/test-utils/identity-fixtures';
import { byTestId, cleanupRenderedComponents, renderComponent } from '$lib/test-utils/svelte-component-test';
import ServicePrincipalEditor from './ServicePrincipalEditor.svelte';

afterEach(cleanupRenderedComponents);

describe('ServicePrincipalEditor', () => {
  it('validates and registers external principal metadata without secret fields', async () => {
    const review = identityAccessReviewFixture();
    const onSave = vi.fn();
    const target = renderComponent(ServicePrincipalEditor, {
      principal: review.principal,
      projects: review.projects,
      onSave,
    });

    expect((byTestId(target, 'service-principal-save') as HTMLButtonElement).disabled).toBe(true);
    setInput(target, 'service-principal-name', 'Workflow bridge');
    setInput(target, 'service-principal-client-id', 'bpane-workflow-bridge');
    setInput(target, 'service-principal-scopes', 'workflow:invoke\nworkflow:read');
    setInput(target, 'service-principal-labels', 'team=automation');
    const project = target.querySelector('[data-testid="service-principal-project-option"]') as HTMLInputElement;
    project.checked = true;
    project.dispatchEvent(new Event('change', { bubbles: true }));
    await tick();

    byTestId(target, 'service-principal-save').click();

    expect(onSave).toHaveBeenCalledWith({
      name: 'Workflow bridge',
      description: null,
      client_id: 'bpane-workflow-bridge',
      issuer: review.principal.issuer,
      labels: { team: 'automation' },
      scopes: ['workflow:invoke', 'workflow:read'],
      allowed_project_ids: ['project-1'],
      state: 'active',
    });
    expect(target.querySelector('input[type="password"]')).toBeNull();
    expect(target.querySelector('input[name*="secret"]')).toBeNull();
  });

  it('edits every persisted field and reports adjacent label validation', async () => {
    const review = identityAccessReviewFixture();
    const onSave = vi.fn();
    const target = renderComponent(ServicePrincipalEditor, {
      principal: review.principal,
      projects: review.projects,
      resource: review.service_principals[0],
      onSave,
    });

    expect((byTestId(target, 'service-principal-client-id') as HTMLInputElement).value).toBe('bpane-mcp-bridge');
    setInput(target, 'service-principal-labels', 'broken');
    await tick();
    expect(byTestId(target, 'service-principal-labels-error').textContent).toContain('key=value');
    expect((byTestId(target, 'service-principal-save') as HTMLButtonElement).disabled).toBe(true);

    setInput(target, 'service-principal-labels', 'purpose=workflow');
    setSelect(target, 'service-principal-state', 'disabled');
    await tick();
    byTestId(target, 'service-principal-save').click();
    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({
      labels: { purpose: 'workflow' },
      state: 'disabled',
    }));
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
