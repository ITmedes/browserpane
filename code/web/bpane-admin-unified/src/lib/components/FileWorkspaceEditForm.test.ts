import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import FileWorkspaceEditForm from './FileWorkspaceEditForm.svelte';

afterEach(cleanupRenderedComponents);

describe('FileWorkspaceEditForm', () => {
  it('validates required fields before enabling create', async () => {
    const onSave = vi.fn();
    const target = renderComponent(FileWorkspaceEditForm, { onSave });

    expect((byTestId(target, 'file-workspace-edit-save') as HTMLButtonElement).disabled).toBe(true);
    await input(target, 'file-workspace-edit-labels', 'broken-label');

    expect(byTestId(target, 'file-workspace-edit-labels-error').textContent).toContain('key=value');
    expect((byTestId(target, 'file-workspace-edit-save') as HTMLButtonElement).disabled).toBe(true);
  });

  it('creates project-scoped requests with parsed labels', async () => {
    const onSave = vi.fn();
    const target = renderComponent(FileWorkspaceEditForm, {
      onSave,
      projectOptionsState: {
        status: 'ready',
        projects: [{ id: 'project-1', name: 'Support', state: 'active' }],
      },
    });

    await input(target, 'file-workspace-edit-name', 'Support inputs');
    await input(target, 'file-workspace-edit-description', 'Reusable files');
    await input(target, 'file-workspace-edit-labels', 'team=support, purpose=demo');
    await select(target, 'file-workspace-edit-project-binding', 'project');
    await select(target, 'file-workspace-edit-project-id', 'project-1');
    byTestId(target, 'file-workspace-edit-save').click();

    expect(onSave).toHaveBeenCalledWith({
      project_id: 'project-1',
      name: 'Support inputs',
      description: 'Reusable files',
      labels: {
        team: 'support',
        purpose: 'demo',
      },
    });
  });

  it('surfaces project option load failures next to the scope control', () => {
    const target = renderComponent(FileWorkspaceEditForm, {
      projectOptionsState: {
        status: 'error',
        message: 'Project catalog unavailable.',
      },
    });

    expect(byTestId(target, 'file-workspace-projects-error').textContent).toContain('Project catalog unavailable');
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
