import { afterEach, describe, expect, it } from 'vitest';

import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import WorkflowCodePreview from './WorkflowCodePreview.svelte';

afterEach(cleanupRenderedComponents);

describe('WorkflowCodePreview', () => {
  it('renders TypeScript-highlighted source content', () => {
    const target = renderComponent(WorkflowCodePreview, {
      state: {
        status: 'ready',
        version: 'v1',
        preview: {
          workflow_definition_id: 'workflow-1',
          workflow_version: 'v1',
          entrypoint: 'dev/workflows/demo/run.ts',
          source: {
            kind: 'git',
            repository_url: '/workspace',
            ref: 'HEAD',
            resolved_commit: 'abc123',
            root_path: 'dev',
          },
          media_type: 'text/typescript; charset=utf-8',
          language: 'typescript',
          content: "export default async function run(): Promise<void> {\n  console.log('ok');\n}\n",
          byte_count: 76,
          max_bytes: 65536,
          truncated: false,
        },
      },
    });

    expect(byTestId(target, 'workflow-code-preview-language').textContent).toContain('TypeScript');
    expect(byTestId(target, 'workflow-code-preview-entrypoint').textContent).toContain('run.ts');
    expect(byTestId(target, 'workflow-code-preview-code-language').className).toContain('language-typescript');
    expect(byTestId(target, 'workflow-code-preview-code').textContent).toContain('export default async function run');
    expect(target.querySelector('.hljs-keyword')?.textContent).toBe('export');
    expect(target.querySelector('.hljs-string')?.textContent).toBe("'ok'");
  });

  it('renders loading, unavailable, and error states', () => {
    let target = renderComponent(WorkflowCodePreview, {
      state: { status: 'loading', version: 'v1' },
    });
    expect(byTestId(target, 'workflow-code-preview-loading').textContent).toContain('Loading source preview');

    void cleanupRenderedComponents();
    target = renderComponent(WorkflowCodePreview, {
      state: { status: 'unavailable', version: 'v1', message: 'No source metadata.' },
    });
    expect(byTestId(target, 'workflow-code-preview-unavailable').textContent).toContain('No source metadata');

    void cleanupRenderedComponents();
    target = renderComponent(WorkflowCodePreview, {
      state: { status: 'error', version: 'v1', message: 'git failed' },
    });
    expect(byTestId(target, 'workflow-code-preview-error').textContent).toContain('git failed');
  });
});
