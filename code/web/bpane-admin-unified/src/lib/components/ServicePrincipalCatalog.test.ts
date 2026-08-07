import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { identityAccessReviewFixture } from '$lib/test-utils/identity-fixtures';
import { byTestId, cleanupRenderedComponents, renderComponent } from '$lib/test-utils/svelte-component-test';
import ServicePrincipalCatalog from './ServicePrincipalCatalog.svelte';

afterEach(cleanupRenderedComponents);

describe('ServicePrincipalCatalog', () => {
  it('filters, selects, and opens contained create/edit views', async () => {
    const review = identityAccessReviewFixture();
    const target = renderComponent(ServicePrincipalCatalog, {
      review,
      onCreate: vi.fn(async () => true),
      onUpdate: vi.fn(async () => true),
    });

    await tick();
    expect(byTestId(target, 'service-principal-count').textContent).toContain('1 of 1');
    expect(byTestId(target, 'service-principal-selected').textContent).toContain('MCP bridge');

    setInput(target, 'service-principal-search', 'missing');
    await tick();
    expect(byTestId(target, 'service-principal-empty').textContent).toContain('No service principals');
    setInput(target, 'service-principal-search', '');
    await tick();

    byTestId(target, 'service-principal-edit').click();
    await tick();
    expect(byTestId(target, 'service-principal-editor-shell').textContent).toContain('Edit service principal');
    byTestId(target, 'service-principal-cancel').click();
    await tick();
    expect(target.querySelector('[data-testid="service-principal-editor-shell"]')).toBeNull();

    byTestId(target, 'service-principal-create').click();
    await tick();
    expect(byTestId(target, 'service-principal-editor-shell').textContent).toContain('Register service principal');
  });

  it('disables the selected principal with a complete replacement request', async () => {
    const onUpdate = vi.fn(async () => true);
    const target = renderComponent(ServicePrincipalCatalog, {
      review: identityAccessReviewFixture(),
      onCreate: vi.fn(async () => true),
      onUpdate,
    });

    await tick();
    byTestId(target, 'service-principal-disable').click();

    expect(onUpdate).toHaveBeenCalledWith('principal-1', expect.objectContaining({
      client_id: 'bpane-mcp-bridge',
      state: 'disabled',
      allowed_project_ids: ['project-1'],
    }));
  });
});

function setInput(target: ParentNode, testId: string, value: string): void {
  const element = byTestId(target, testId) as HTMLInputElement;
  element.value = value;
  element.dispatchEvent(new Event('input', { bubbles: true }));
}
