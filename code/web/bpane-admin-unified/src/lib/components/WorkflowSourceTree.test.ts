import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import type { WorkflowDefinitionSourceFileResource } from '$lib/workflows/workflow-types';
import WorkflowSourceTree from './WorkflowSourceTree.svelte';

afterEach(cleanupRenderedComponents);

describe('WorkflowSourceTree', () => {
  it('renders an expandable folder tree and selects nested files', async () => {
    const onSelectFile = vi.fn();
    const target = renderComponent(WorkflowSourceTree, {
      files: sourceFiles(),
      selectedPath: 'dev/workflows/demo/run.ts',
      onSelectFile,
      formatFileMeta: (file: WorkflowDefinitionSourceFileResource) => `${file.language} file`,
    });

    expect(folder(target, 'dev').getAttribute('aria-expanded')).toBe('true');
    expect(folder(target, 'dev/workflows').getAttribute('aria-expanded')).toBe('true');
    expect(folder(target, 'dev/workflows/demo').getAttribute('aria-expanded')).toBe('true');
    expect(file(target, 'dev/workflows/demo/run.ts').getAttribute('data-selected')).toBe('true');
    expect(file(target, 'dev/workflows/demo/helper.ts').textContent).toContain('helper.ts');

    file(target, 'dev/workflows/demo/helper.ts').click();

    expect(onSelectFile).toHaveBeenCalledWith('dev/workflows/demo/helper.ts');

    folder(target, 'dev/web-fixtures').click();

    await vi.waitFor(() => {
      expect(file(target, 'dev/web-fixtures/test-embed.html').textContent).toContain('test-embed.html');
    });
  });
});

function folder(target: ParentNode, path: string): HTMLButtonElement {
  const element = target.querySelector(`[data-testid="workflow-code-folder-row"][data-source-path="${path}"]`);
  expect(element).toBeInstanceOf(HTMLButtonElement);
  return element as HTMLButtonElement;
}

function file(target: ParentNode, path: string): HTMLButtonElement {
  const element = target.querySelector(`[data-testid="workflow-code-file-row"][data-source-path="${path}"]`);
  expect(element).toBeInstanceOf(HTMLButtonElement);
  return element as HTMLButtonElement;
}

function sourceFiles(): readonly WorkflowDefinitionSourceFileResource[] {
  return [
    {
      path: 'dev/workflows/demo/run.ts',
      byte_count: 76,
      media_type: 'text/typescript; charset=utf-8',
      language: 'typescript',
      entrypoint: true,
    },
    {
      path: 'dev/workflows/demo/helper.ts',
      byte_count: 34,
      media_type: 'text/typescript; charset=utf-8',
      language: 'typescript',
      entrypoint: false,
    },
    {
      path: 'dev/web-fixtures/test-embed.html',
      byte_count: 120,
      media_type: 'text/plain; charset=utf-8',
      language: 'text',
      entrypoint: false,
    },
  ];
}
