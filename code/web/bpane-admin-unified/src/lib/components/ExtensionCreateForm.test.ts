import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { byTestId, cleanupRenderedComponents, renderComponent } from '$lib/test-utils/svelte-component-test';
import ExtensionCreateForm from './ExtensionCreateForm.svelte';

afterEach(cleanupRenderedComponents);

describe('ExtensionCreateForm', () => {
  it('keeps submission disabled until required fields are valid', async () => {
    const target = renderComponent(ExtensionCreateForm);
    expect((byTestId(target, 'extension-create-submit') as HTMLButtonElement).disabled).toBe(true);
    await input(target, 'extension-create-name', 'Login helper');
    await input(target, 'extension-create-labels', 'invalid');
    expect(byTestId(target, 'extension-create-labels-error').textContent).toContain('key=value');
    expect((byTestId(target, 'extension-create-submit') as HTMLButtonElement).disabled).toBe(true);
  });

  it('submits normalized definition metadata', async () => {
    const onSave = vi.fn();
    const target = renderComponent(ExtensionCreateForm, { onSave });
    await input(target, 'extension-create-name', ' Login helper ');
    await input(target, 'extension-create-description', ' Approved helper ');
    await input(target, 'extension-create-labels', 'team=platform');
    byTestId(target, 'extension-create-submit').click();

    expect(onSave).toHaveBeenCalledWith({
      name: 'Login helper',
      description: 'Approved helper',
      labels: { team: 'platform' },
    });
  });
});

async function input(target: HTMLElement, testId: string, value: string): Promise<void> {
  const element = byTestId(target, testId) as HTMLInputElement | HTMLTextAreaElement;
  element.value = value;
  element.dispatchEvent(new Event('input', { bubbles: true }));
  await tick();
}
