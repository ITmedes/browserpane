import { describe, expect, it } from 'vitest';

import {
  buildFileWorkspaceOverviewModel,
  fileWorkspaceFileRow,
  fileWorkspaceMatchesSearch,
  labelSummary,
  provenanceSummary,
} from './file-workspace-overview-view-model';
import type {
  FileWorkspaceFileResource,
  FileWorkspaceResource,
} from './file-workspace-types';

describe('file workspace overview view model', () => {
  it('summarizes owner/project scope and known file counts', () => {
    const model = buildFileWorkspaceOverviewModel([
      workspace({ id: 'owner-workspace', project: false, labels: { team: 'platform' } }),
      workspace({ id: 'project-workspace', project: true }),
    ], {
      'owner-workspace': 2,
      'project-workspace': null,
    });

    expect(model.metrics.map((metric) => [metric.label, metric.value])).toEqual([
      ['Workspaces', '2'],
      ['Owner scoped', '1'],
      ['Project scoped', '1'],
      ['Known files', '2'],
    ]);
    expect(model.rows[0]).toMatchObject({
      id: 'owner-workspace',
      scope: 'Owner scoped',
      fileCountLabel: '2 files',
      labels: 'team=platform',
    });
    expect(model.rows[1]).toMatchObject({
      id: 'project-workspace',
      scope: 'Support',
      fileCountLabel: 'files unavailable',
    });
  });

  it('matches catalog rows by scope, labels, and file-count badge text', () => {
    const [row] = buildFileWorkspaceOverviewModel([
      workspace({ id: 'workspace-1', project: true, labels: { purpose: 'demo' } }),
    ], {
      'workspace-1': 1,
    }).rows;

    expect(fileWorkspaceMatchesSearch(row!, 'support')).toBe(true);
    expect(fileWorkspaceMatchesSearch(row!, 'purpose=demo')).toBe(true);
    expect(fileWorkspaceMatchesSearch(row!, '1 file')).toBe(true);
    expect(fileWorkspaceMatchesSearch(row!, 'missing')).toBe(false);
  });

  it('formats file rows, label summaries, and provenance summaries', () => {
    const row = fileWorkspaceFileRow(file());

    expect(row).toMatchObject({
      id: 'file-1',
      workspaceId: 'workspace-1',
      name: 'fixture.csv',
      mediaType: 'text/csv',
      size: '2.0 KB',
      digest: 'sha256 0123456789ab...cdef',
      provenance: 'source=test',
    });
    expect(labelSummary({ z: 'last', a: 'first' })).toBe('a=first, z=last');
    expect(provenanceSummary(null)).toBe('Provenance unavailable');
  });
});

function workspace(options: {
  readonly id: string;
  readonly project: boolean;
  readonly labels?: Readonly<Record<string, string>>;
}): FileWorkspaceResource {
  return {
    id: options.id,
    project_id: options.project ? 'project-1' : null,
    project: options.project ? { id: 'project-1', name: 'Support', state: 'active' } : null,
    name: options.id,
    description: null,
    labels: options.labels ?? {},
    files_path: `/api/v1/file-workspaces/${options.id}/files`,
    created_at: '2026-06-20T09:00:00.000Z',
    updated_at: '2026-06-20T10:00:00.000Z',
  };
}

function file(): FileWorkspaceFileResource {
  return {
    id: 'file-1',
    workspace_id: 'workspace-1',
    name: 'fixture.csv',
    media_type: 'text/csv',
    byte_count: 2048,
    sha256_hex: '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
    provenance: { source: 'test' },
    content_path: '/api/v1/file-workspaces/workspace-1/files/file-1/content',
    created_at: '2026-06-20T09:30:00.000Z',
    updated_at: '2026-06-20T09:30:00.000Z',
  };
}
