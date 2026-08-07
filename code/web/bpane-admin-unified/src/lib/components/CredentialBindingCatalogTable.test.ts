import { tick } from 'svelte';
import { afterEach, describe, expect, it } from 'vitest';
import type { CredentialBindingResource } from '$lib/credential-bindings/credential-binding-types';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import CredentialBindingCatalogTable from './CredentialBindingCatalogTable.svelte';

afterEach(cleanupRenderedComponents);

describe('CredentialBindingCatalogTable', () => {
  it('filters bindings by scope and search', async () => {
    const target = renderComponent(CredentialBindingCatalogTable, {
      bindings: [binding('owner', false, 'form_fill'), binding('project', true, 'totp_fill')],
    });
    expect(target.querySelectorAll('[data-testid="credential-bindings-list-row"]')).toHaveLength(2);
    byTestId(target, 'credential-bindings-lens-project').click();
    await tick();
    expect(target.querySelectorAll('[data-testid="credential-bindings-list-row"]')).toHaveLength(1);
    byTestId(target, 'credential-bindings-lens-all').click();
    await input(target, 'credential-bindings-search', 'form fill');
    expect(target.querySelectorAll('[data-testid="credential-bindings-list-row"]')).toHaveLength(1);
    expect(target.textContent).toContain('owner');
  });
});

async function input(target: HTMLElement, testId: string, value: string) {
  const element = byTestId(target, testId) as HTMLInputElement;
  element.value = value;
  element.dispatchEvent(new Event('input', { bubbles: true }));
  await tick();
}
function binding(
  id: string,
  project: boolean,
  mode: CredentialBindingResource['injection_mode'],
): CredentialBindingResource {
  return {
    id,
    project_id: project ? 'project-1' : null,
    project: project ? { id: 'project-1', name: 'Support', state: 'active' } : null,
    name: id,
    provider: 'vault_kv_v2',
    external_ref: `secret/data/${id}`,
    namespace: null,
    allowed_origins: ['https://support.example'],
    injection_mode: mode,
    totp: null,
    labels: {},
    created_at: '2026-08-07T08:00:00.000Z',
    updated_at: '2026-08-07T09:00:00.000Z',
  };
}
