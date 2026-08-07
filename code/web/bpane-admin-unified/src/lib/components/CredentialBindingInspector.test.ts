import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import CredentialBindingInspector from './CredentialBindingInspector.svelte';

afterEach(cleanupRenderedComponents);
describe('CredentialBindingInspector', () => {
  it('renders safe provider metadata and never secret payloads', () => {
    const target = renderComponent(CredentialBindingInspector, {
      state: { status: 'ready', binding: bindingPayload() },
    });
    expect(byTestId(target, 'credential-binding-detail-name').textContent).toContain(
      'Support login',
    );
    expect(byTestId(target, 'credential-binding-write-only').textContent).toContain(
      'cannot be retrieved',
    );
    expect(byTestId(target, 'credential-binding-detail-external-ref').textContent).toContain(
      'secret/data/binding-1',
    );
    expect(target.textContent).not.toContain('must-not-leak');
  });
  it('delegates refresh', () => {
    const onRefresh = vi.fn();
    const target = renderComponent(CredentialBindingInspector, {
      state: { status: 'ready', binding: bindingPayload() },
      onRefresh,
    });
    byTestId(target, 'credential-binding-refresh-detail').click();
    expect(onRefresh).toHaveBeenCalledOnce();
  });
});
function bindingPayload() {
  return {
    id: 'binding-1',
    project_id: null,
    project: null,
    name: 'Support login',
    provider: 'vault_kv_v2' as const,
    external_ref: 'secret/data/binding-1',
    namespace: 'support',
    allowed_origins: ['https://support.example'],
    injection_mode: 'form_fill' as const,
    totp: null,
    labels: { team: 'support' },
    created_at: '2026-08-07T08:00:00.000Z',
    updated_at: '2026-08-07T09:00:00.000Z',
  };
}
