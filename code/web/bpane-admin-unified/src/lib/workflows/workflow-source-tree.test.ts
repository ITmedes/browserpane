import { describe, expect, it } from 'vitest';

import {
  buildWorkflowSourceTree,
  flattenWorkflowSourceTree,
  sourcePathDirectoryAncestors,
  sourceTreeDirectoryPaths,
} from './workflow-source-tree';
import type { WorkflowDefinitionSourceFileResource } from './workflow-types';

describe('workflow source tree view model', () => {
  it('groups source files into sorted directories and files', () => {
    const tree = buildWorkflowSourceTree([
      file('dev/workflows/demo/run.ts', true),
      file('dev/web-fixtures/test-embed.html'),
      file('dev/workflows/demo/helper.ts'),
      file('README.md'),
    ]);

    expect(tree.map((node) => node.path)).toEqual(['dev', 'README.md']);
    expect(sourceTreeDirectoryPaths(tree)).toEqual(['dev', 'dev/web-fixtures', 'dev/workflows', 'dev/workflows/demo']);
    const rows = flattenWorkflowSourceTree(
      tree,
      new Set(['dev', 'dev/workflows', 'dev/workflows/demo']),
    );

    expect(rows.map((row) => `${row.depth}:${row.node.path}`)).toEqual([
      '0:dev',
      '1:dev/web-fixtures',
      '1:dev/workflows',
      '2:dev/workflows/demo',
      '3:dev/workflows/demo/helper.ts',
      '3:dev/workflows/demo/run.ts',
      '0:README.md',
    ]);
  });

  it('derives ancestors for selected file expansion', () => {
    expect(sourcePathDirectoryAncestors('dev/workflows/demo/run.ts')).toEqual([
      'dev',
      'dev/workflows',
      'dev/workflows/demo',
    ]);
    expect(sourcePathDirectoryAncestors(null)).toEqual([]);
  });
});

function file(path: string, entrypoint = false): WorkflowDefinitionSourceFileResource {
  return {
    path,
    byte_count: 12,
    media_type: 'text/plain; charset=utf-8',
    language: 'text',
    entrypoint,
  };
}
