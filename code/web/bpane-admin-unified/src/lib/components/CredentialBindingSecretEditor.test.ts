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

describe('CredentialBindingSecretEditor', () => {
  it('switches from a payload to an opaque provider reference', async () => {
    const draft = createCredentialBindingDraft();
    const target = renderComponent(CredentialBindingEditorTestHarness, {
      editor: 'secret',
      initialDraft: draft,
      validation: { valid: true, request: null, fieldErrors: {} },
    });

    expect(byTestId(target, 'credential-binding-secret-payload')).toBeInstanceOf(HTMLElement);
    byTestId(target, 'credential-binding-secret-reference-mode').click();
    await tick();

    expect(byTestId(target, 'credential-binding-external-ref')).toBeInstanceOf(HTMLElement);
  });
});
