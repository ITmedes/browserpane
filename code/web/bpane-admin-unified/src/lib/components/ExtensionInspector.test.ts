import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { ExtensionDefinitionResource } from '$lib/extensions/extension-types';
import { byTestId, cleanupRenderedComponents, renderComponent } from '$lib/test-utils/svelte-component-test';
import ExtensionInspector from './ExtensionInspector.svelte';

afterEach(cleanupRenderedComponents);

describe('ExtensionInspector', () => {
  it('renders safe metadata and delegates enablement transitions', () => {
    const onSetEnabled = vi.fn();
    const target = renderComponent(ExtensionInspector, {
      state: { status: 'ready', extension: extension(false) },
      onSetEnabled,
    });

    expect(byTestId(target, 'extension-detail-name').textContent).toContain('Login helper');
    expect(byTestId(target, 'extension-detail-state').textContent).toContain('Disabled');
    expect(byTestId(target, 'extension-detail-version').textContent).toContain('Not published');
    byTestId(target, 'extension-toggle-enabled').click();
    expect(onSetEnabled).toHaveBeenCalledWith(true);
  });

  it('validates and delegates version publication', async () => {
    const onPublishVersion = vi.fn();
    const target = renderComponent(ExtensionInspector, {
      state: { status: 'ready', extension: extension(true) },
      onPublishVersion,
    });
    await input(target, 'extension-version-value', '1.0.0');
    await input(target, 'extension-version-path', 'relative/path');
    expect(byTestId(target, 'extension-version-path-error').textContent).toContain('absolute');
    await input(target, 'extension-version-path', '/opt/extensions/login');
    byTestId(target, 'extension-version-submit').click();
    expect(onPublishVersion).toHaveBeenCalledWith({ version: '1.0.0', install_path: '/opt/extensions/login' });
  });
});

async function input(target: HTMLElement, testId: string, value: string): Promise<void> {
  const element = byTestId(target, testId) as HTMLInputElement;
  element.value = value;
  element.dispatchEvent(new Event('input', { bubbles: true }));
  await tick();
}

function extension(enabled: boolean): ExtensionDefinitionResource {
  return {
    id: 'extension-1',
    name: 'Login helper',
    description: 'Approved helper',
    enabled,
    latest_version: null,
    labels: { team: 'platform' },
    created_at: '2026-08-07T08:00:00.000Z',
    updated_at: '2026-08-07T09:00:00.000Z',
  };
}
