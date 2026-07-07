import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import EgressProfileEditForm from './EgressProfileEditForm.svelte';

afterEach(cleanupRenderedComponents);

describe('EgressProfileEditForm', () => {
  it('validates TLS requirements and submits a project-scoped request', async () => {
    const onSave = vi.fn();
    const target = renderComponent(EgressProfileEditForm, {
      mode: 'create',
      projectOptionsState: {
        status: 'ready',
        projects: [{ id: 'project-1', name: 'Support', state: 'active' }],
      },
      onSave,
    });

    await input(target, 'egress-profile-edit-name', 'Support TLS');
    await select(target, 'egress-profile-edit-project-binding', 'project');
    await select(target, 'egress-profile-edit-project-id', 'project-1');
    await select(target, 'egress-profile-edit-observation-mode', 'tls_intercept');

    expect(byTestId(target, 'egress-profile-edit-tls-requirements').textContent).toContain('TLS intercept requirements');
    expect(byTestId(target, 'egress-profile-edit-proxy-url-error').textContent).toContain('Proxy URL is required');
    expect((byTestId(target, 'egress-profile-edit-save') as HTMLButtonElement).disabled).toBe(true);

    await input(target, 'egress-profile-edit-proxy-url', 'http://proxy.example:3128');
    await input(target, 'egress-profile-edit-proxy-credential-binding-id', '33333333-3333-4333-8333-333333333333');
    await input(target, 'egress-profile-edit-bypass-rules', 'localhost\n127.0.0.1');
    await input(target, 'egress-profile-edit-custom-ca-certificate-ref', 'file:///workspace/dev/egress-ca.pem');
    await input(target, 'egress-profile-edit-custom-ca-display-name', 'Local CA');
    await input(target, 'egress-profile-edit-sensitive-log-sink-ref', 'siem://browserpane/support');
    await input(target, 'egress-profile-edit-sensitive-log-sink-display-name', 'Support SIEM');
    await input(target, 'egress-profile-edit-labels', 'team=support');

    const save = byTestId(target, 'egress-profile-edit-save') as HTMLButtonElement;
    expect(save.disabled).toBe(false);
    save.click();

    expect(onSave).toHaveBeenCalledWith({
      project_id: 'project-1',
      name: 'Support TLS',
      description: null,
      labels: { team: 'support' },
      proxy: {
        url: 'http://proxy.example:3128',
        credential_binding_id: '33333333-3333-4333-8333-333333333333',
      },
      bypass_rules: ['127.0.0.1', 'localhost'],
      custom_ca: {
        certificate_ref: 'file:///workspace/dev/egress-ca.pem',
        display_name: 'Local CA',
      },
      traffic_observation: {
        mode: 'tls_intercept',
        sensitive_log_sink_ref: 'siem://browserpane/support',
        sensitive_log_sink_display_name: 'Support SIEM',
      },
      state: 'ready',
    });
  });
});

async function input(target: HTMLElement, testId: string, value: string): Promise<void> {
  const element = byTestId(target, testId) as HTMLInputElement | HTMLTextAreaElement;
  element.value = value;
  element.dispatchEvent(new Event('input', { bubbles: true }));
  await tick();
}

async function select(target: HTMLElement, testId: string, value: string): Promise<void> {
  const element = byTestId(target, testId) as HTMLSelectElement;
  element.value = value;
  element.dispatchEvent(new Event('change', { bubbles: true }));
  await tick();
}
