import { afterEach, describe, expect, it, vi } from 'vitest';
import { byTestId, cleanupRenderedComponents, renderComponent } from '$lib/test-utils/svelte-component-test';
import CredentialBindingOverview from './CredentialBindingOverview.svelte';

afterEach(cleanupRenderedComponents);
describe('CredentialBindingOverview', () => {
  it('renders loading, error, empty, and ready states', async () => {
    let target = renderComponent(CredentialBindingOverview, { state: { status: 'loading' } }); expect(byTestId(target, 'credential-bindings-loading')).toBeInstanceOf(HTMLElement);
    await cleanupRenderedComponents(); target = renderComponent(CredentialBindingOverview, { state: { status: 'error', message: 'Unavailable.' } }); expect(byTestId(target, 'credential-bindings-error').textContent).toContain('Unavailable');
    await cleanupRenderedComponents(); target = renderComponent(CredentialBindingOverview, { state: { status: 'ready', bindings: [] } }); expect(byTestId(target, 'credential-bindings-empty')).toBeInstanceOf(HTMLElement);
    await cleanupRenderedComponents(); target = renderComponent(CredentialBindingOverview, { state: { status: 'ready', bindings: [bindingPayload()] } }); expect(byTestId(target, 'credential-bindings-metric-total').textContent).toContain('1');
  });
  it('delegates refresh', () => { const onRefresh = vi.fn(); const target = renderComponent(CredentialBindingOverview, { state: { status: 'ready', bindings: [bindingPayload()] }, onRefresh }); byTestId(target, 'credential-bindings-refresh-button').click(); expect(onRefresh).toHaveBeenCalledOnce(); });
});
function bindingPayload() { return { id: 'binding-1', project_id: null, project: null, name: 'Support login', provider: 'vault_kv_v2' as const, external_ref: 'secret/data/binding-1', namespace: null, allowed_origins: ['https://support.example'], injection_mode: 'form_fill' as const, totp: null, labels: {}, created_at: '2026-08-07T08:00:00.000Z', updated_at: '2026-08-07T09:00:00.000Z' }; }
