import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import ExtensionVersionPublisher from './ExtensionVersionPublisher.svelte';

afterEach(cleanupRenderedComponents);

describe('ExtensionVersionPublisher', () => {
  it('validates absolute paths and delegates valid versions', async () => {
    const onPublish = vi.fn();
    const target = renderComponent(ExtensionVersionPublisher, { onPublish });

    await input(target, 'extension-version-value', '1.0.0');
    await input(target, 'extension-version-path', 'relative');
    expect(byTestId(target, 'extension-version-path-error').textContent).toContain('absolute');

    await input(target, 'extension-version-path', '/opt/extensions/login');
    byTestId(target, 'extension-version-submit').click();
    expect(onPublish).toHaveBeenCalledWith({
      version: '1.0.0',
      install_path: '/opt/extensions/login',
    });
  });
});

async function input(target: HTMLElement, testId: string, value: string): Promise<void> {
  const element = byTestId(target, testId) as HTMLInputElement;
  element.value = value;
  element.dispatchEvent(new Event('input', { bubbles: true }));
  await tick();
}
