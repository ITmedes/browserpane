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

describe('CredentialBindingIdentityEditor', () => {
  it('shows active project choices and project validation', async () => {
    const draft = createCredentialBindingDraft();
    const target = renderComponent(CredentialBindingEditorTestHarness, {
      editor: 'identity',
      initialDraft: draft,
      validation: {
        valid: false,
        request: null,
        fieldErrors: { projectId: ['Select a project.'] },
      },
      projectOptionsState: {
        status: 'ready',
        projects: [
          { id: 'active', name: 'Active project', state: 'active' },
          { id: 'disabled', name: 'Disabled project', state: 'disabled' },
        ],
      },
    });

    byTestId(target, 'credential-binding-scope-project').click();
    await tick();

    expect(byTestId(target, 'credential-binding-project-error').textContent).toContain(
      'Select a project',
    );
    expect(target.textContent).toContain('Active project');
    expect(target.textContent).not.toContain('Disabled project');
  });
});
