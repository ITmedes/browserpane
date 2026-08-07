import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { identityAccessReviewFixture } from '$lib/test-utils/identity-fixtures';
import { byTestId, cleanupRenderedComponents, renderComponent } from '$lib/test-utils/svelte-component-test';
import IdentityMappingCatalog from './IdentityMappingCatalog.svelte';

afterEach(cleanupRenderedComponents);

describe('IdentityMappingCatalog', () => {
  it('filters, selects, and opens contained create/edit views', async () => {
    const target = renderComponent(IdentityMappingCatalog, {
      review: identityAccessReviewFixture(),
      onCreate: vi.fn(async () => true),
      onUpdate: vi.fn(async () => true),
    });

    await tick();
    expect(byTestId(target, 'identity-mapping-count').textContent).toContain('1 of 1');
    expect(byTestId(target, 'identity-mapping-selected').textContent).toContain('Demo operator access');

    setInput(target, 'identity-mapping-search', 'missing');
    await tick();
    expect(byTestId(target, 'identity-mapping-empty').textContent).toContain('No identity mappings');
    setInput(target, 'identity-mapping-search', '');
    await tick();

    byTestId(target, 'identity-mapping-edit').click();
    await tick();
    expect(byTestId(target, 'identity-mapping-editor-shell').textContent).toContain('Edit identity mapping');
    byTestId(target, 'identity-mapping-cancel').click();
    await tick();
    expect(target.querySelector('[data-testid="identity-mapping-editor-shell"]')).toBeNull();

    byTestId(target, 'identity-mapping-create').click();
    await tick();
    expect(byTestId(target, 'identity-mapping-editor-shell').textContent).toContain('Create identity mapping');
  });

  it('disables the selected mapping with a kind-safe replacement request', async () => {
    const onUpdate = vi.fn(async () => true);
    const target = renderComponent(IdentityMappingCatalog, {
      review: identityAccessReviewFixture(),
      onCreate: vi.fn(async () => true),
      onUpdate,
    });

    await tick();
    byTestId(target, 'identity-mapping-disable').click();

    expect(onUpdate).toHaveBeenCalledWith('mapping-1', expect.objectContaining({
      kind: 'user',
      external_id: 'user-1',
      project_id: 'project-1',
      claim_name: null,
      service_principal_id: null,
      state: 'disabled',
    }));
  });
});

function setInput(target: ParentNode, testId: string, value: string): void {
  const element = byTestId(target, testId) as HTMLInputElement;
  element.value = value;
  element.dispatchEvent(new Event('input', { bubbles: true }));
}
