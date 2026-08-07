import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { byTestId, cleanupRenderedComponents, renderComponent } from '$lib/test-utils/svelte-component-test';
import CredentialBindingCreateForm from './CredentialBindingCreateForm.svelte';

afterEach(cleanupRenderedComponents);
describe('CredentialBindingCreateForm', () => {
  it('validates secret JSON, origins, and labels near their controls', async () => {
    const target = renderComponent(CredentialBindingCreateForm);
    await input(target, 'credential-binding-name', 'Support login');
    await input(target, 'credential-binding-origins', 'not-a-url');
    await input(target, 'credential-binding-secret-payload', '[]');
    await input(target, 'credential-binding-labels', 'broken');
    expect(byTestId(target, 'credential-binding-origins-error').textContent).toContain('HTTP or HTTPS');
    expect(byTestId(target, 'credential-binding-secret-payload-error').textContent).toContain('JSON object');
    expect(byTestId(target, 'credential-binding-labels-error').textContent).toContain('key=value');
  });

  it('submits project-scoped write-only payloads and TOTP metadata', async () => {
    const onSave = vi.fn();
    const target = renderComponent(CredentialBindingCreateForm, { onSave, projectOptionsState: { status: 'ready', projects: [{ id: 'project-1', name: 'Support', state: 'active' }] } });
    await input(target, 'credential-binding-name', 'Support TOTP');
    byTestId(target, 'credential-binding-scope-project').click(); await tick();
    await select(target, 'credential-binding-project', 'project-1');
    await select(target, 'credential-binding-injection-mode', 'totp_fill');
    await input(target, 'credential-binding-origins', 'https://support.example');
    await input(target, 'credential-binding-secret-payload', '{"secret":"sensitive","selector":"#otp"}');
    await input(target, 'credential-binding-totp-issuer', 'BrowserPane');
    byTestId(target, 'credential-binding-create-submit').click();
    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({ project_id: 'project-1', injection_mode: 'totp_fill', secret_payload: { secret: 'sensitive', selector: '#otp' }, totp: expect.objectContaining({ issuer: 'BrowserPane', digits: 6 }) }));
  });

  it('supports opaque provider references without a secret payload', async () => {
    const onSave = vi.fn(); const target = renderComponent(CredentialBindingCreateForm, { onSave });
    await input(target, 'credential-binding-name', 'Existing'); byTestId(target, 'credential-binding-secret-reference-mode').click(); await tick(); await input(target, 'credential-binding-external-ref', 'secret/data/existing'); byTestId(target, 'credential-binding-create-submit').click();
    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({ external_ref: 'secret/data/existing' }));
    expect(onSave.mock.calls[0]?.[0]).not.toHaveProperty('secret_payload');
  });
});
async function input(target: HTMLElement, testId: string, value: string) { const element = byTestId(target, testId) as HTMLInputElement | HTMLTextAreaElement; element.value = value; element.dispatchEvent(new Event('input', { bubbles: true })); await tick(); }
async function select(target: HTMLElement, testId: string, value: string) { const element = byTestId(target, testId) as HTMLSelectElement; element.value = value; element.dispatchEvent(new Event('change', { bubbles: true })); await tick(); }
