import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import { createNewBrowserContextEditDraft } from '$lib/browser-contexts/browser-context-edit-view-model';
import BrowserContextEditForm from './BrowserContextEditForm.svelte';

afterEach(cleanupRenderedComponents);

describe('BrowserContextEditForm', () => {
  it('renders validation beside fields and submits a project-scoped context', async () => {
    const onSave = vi.fn();
    const target = renderComponent(BrowserContextEditForm, {
      projectOptionsState: {
        status: 'ready',
        projects: [{ id: 'project-1', name: 'Support', state: 'active' }],
      },
      onSave,
    });

    byTestId(target, 'browser-context-edit-save').click();
    expect(onSave).not.toHaveBeenCalled();

    await input(target, 'browser-context-edit-name', 'Support baseline');
    await textarea(target, 'browser-context-edit-labels', 'team=support');
    await select(target, 'browser-context-edit-project-binding', 'project');
    await select(target, 'browser-context-edit-project-id', 'project-1');
    await checkbox(target, 'browser-context-edit-storage-limit-enabled', true);
    await input(target, 'browser-context-edit-max-profile-storage-bytes', '1073741824');

    byTestId(target, 'browser-context-edit-save').click();

    expect(onSave).toHaveBeenCalledWith({
      project_id: 'project-1',
      name: 'Support baseline',
      description: null,
      labels: { team: 'support' },
      persistence_mode: 'reusable',
      retention_sec: 604800,
      max_profile_storage_bytes: 1073741824,
    });
  });

  it('shows project option loading and error messages inline', async () => {
    let target = renderComponent(BrowserContextEditForm, {
      projectOptionsState: { status: 'loading' },
    });
    expect(byTestId(target, 'browser-context-projects-loading').textContent).toContain('Loading projects');

    await cleanupRenderedComponents();
    target = renderComponent(BrowserContextEditForm, {
      projectOptionsState: { status: 'error', message: 'No active admin access token is available.' },
    });
    expect(byTestId(target, 'browser-context-projects-error').textContent).toContain('Project options unavailable');
  });

  it('supports prefilled lifecycle operations, locked persistence, and external blockers', async () => {
    const onSave = vi.fn();
    const initialDraft = {
      ...createNewBrowserContextEditDraft(),
      name: 'Support baseline copy',
      description: 'Copied profile',
    };
    let target = renderComponent(BrowserContextEditForm, {
      initialDraft,
      title: 'Clone browser context settings',
      submitLabel: 'Clone browser context',
      persistenceLocked: true,
      requireChanges: false,
      submitBlocked: true,
      submitBlockedHint: 'Select an archive before importing.',
      onSave,
    });

    expect(target.textContent).toContain('Clone browser context settings');
    expect(target.textContent).toContain('Clone browser context');
    expect((byTestId(target, 'browser-context-edit-persistence-mode') as HTMLSelectElement).disabled).toBe(true);
    expect(byTestId(target, 'browser-context-edit-submit-blocked').textContent).toContain(
      'Select an archive before importing.',
    );
    expect((byTestId(target, 'browser-context-edit-save') as HTMLButtonElement).disabled).toBe(true);

    await cleanupRenderedComponents();
    target = renderComponent(BrowserContextEditForm, {
      initialDraft,
      submitLabel: 'Clone browser context',
      persistenceLocked: true,
      requireChanges: false,
      onSave,
    });
    byTestId(target, 'browser-context-edit-save').click();

    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({
      name: 'Support baseline copy',
      persistence_mode: 'reusable',
    }));
  });
});

async function input(target: HTMLElement, testId: string, value: string): Promise<void> {
  const element = byTestId(target, testId) as HTMLInputElement;
  element.value = value;
  element.dispatchEvent(new Event('input', { bubbles: true }));
  await tick();
}

async function textarea(target: HTMLElement, testId: string, value: string): Promise<void> {
  const element = byTestId(target, testId) as HTMLTextAreaElement;
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

async function checkbox(target: HTMLElement, testId: string, value: boolean): Promise<void> {
  const element = byTestId(target, testId) as HTMLInputElement;
  element.checked = value;
  element.dispatchEvent(new Event('change', { bubbles: true }));
  await tick();
}
