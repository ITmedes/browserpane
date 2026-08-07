import { tick } from 'svelte';
import { afterEach, describe, expect, it } from 'vitest';
import { createCredentialBindingDraft } from '$lib/credential-bindings/credential-binding-form-model';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import CredentialBindingEditorTestHarness from '$lib/test-utils/CredentialBindingEditorTestHarness.svelte';

afterEach(cleanupRenderedComponents);

describe('CredentialBindingInjectionEditor', () => {
  it('reveals TOTP controls and renders field validation', async () => {
    const draft = createCredentialBindingDraft();
    const target = renderComponent(CredentialBindingEditorTestHarness, {
      editor: 'injection',
      initialDraft: draft,
      validation: {
        valid: false,
        request: null,
        fieldErrors: { totpDigits: ['TOTP digits must be a positive integer.'] },
      },
    });
    const select = byTestId(target, 'credential-binding-injection-mode') as HTMLSelectElement;
    select.value = 'totp_fill';
    select.dispatchEvent(new Event('change', { bubbles: true }));
    await tick();

    expect(byTestId(target, 'credential-binding-totp-fields')).toBeInstanceOf(HTMLElement);
    expect(byTestId(target, 'credential-binding-totp-digits-error').textContent).toContain(
      'positive integer',
    );
  });
});
