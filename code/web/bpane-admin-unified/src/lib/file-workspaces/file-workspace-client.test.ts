import { describe, expect, it, vi } from 'vitest';

import {
  FileWorkspaceCatalogClient,
  FileWorkspaceCatalogError,
  toFileWorkspaceListResponse,
} from './file-workspace-client';

describe('FileWorkspaceCatalogClient', () => {
  it('loads file workspaces through the authenticated control API', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () =>
      jsonResponse({ workspaces: [workspacePayload()] }, 200));
    const client = new FileWorkspaceCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'token-1',
      fetchImpl,
    });

    const response = await client.listFileWorkspaces();

    expect(response.workspaces[0]).toMatchObject({
      id: 'workspace-1',
      project_id: 'project-1',
      name: 'Support inputs',
      files_path: '/api/v1/file-workspaces/workspace-1/files',
    });
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL('http://browserpane.test/api/v1/file-workspaces'),
      expect.objectContaining({ method: 'GET' }),
    );
    const headers = fetchImpl.mock.calls[0]?.[1]?.headers as Headers;
    expect(headers.get('authorization')).toBe('Bearer token-1');
  });

  it('delegates authentication failures', async () => {
    const onAuthenticationFailure = vi.fn();
    const client = new FileWorkspaceCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'expired-token',
      fetchImpl: async () => new Response('unauthorized', { status: 401 }),
      onAuthenticationFailure,
    });

    await expect(client.listFileWorkspaces()).rejects.toMatchObject({
      code: 'http_error',
      status: 401,
    });
    expect(onAuthenticationFailure).toHaveBeenCalledOnce();
  });

  it('creates workspaces and manages workspace files', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/api/v1/file-workspaces') && init?.method === 'POST') {
        return jsonResponse(workspacePayload(), 201);
      }
      if (url.endsWith('/api/v1/file-workspaces/workspace-1') && init?.method === 'GET') {
        return jsonResponse(workspacePayload(), 200);
      }
      if (url.endsWith('/api/v1/file-workspaces/workspace-1/files') && init?.method === 'GET') {
        return jsonResponse({ files: [filePayload()] }, 200);
      }
      if (url.endsWith('/api/v1/file-workspaces/workspace-1/files') && init?.method === 'POST') {
        return jsonResponse(filePayload({ name: 'uploaded.csv' }), 201);
      }
      if (url.endsWith('/api/v1/file-workspaces/workspace-1/files/file-1/content')) {
        return new Response('downloaded content', { status: 200 });
      }
      if (url.endsWith('/api/v1/file-workspaces/workspace-1/files/file-1') && init?.method === 'DELETE') {
        return jsonResponse(filePayload(), 200);
      }
      return new Response('not found', { status: 404 });
    });
    const client = new FileWorkspaceCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'token-1',
      fetchImpl,
    });
    const createRequest = {
      project_id: 'project-1',
      name: 'Support inputs',
      description: 'Reusable customer data',
      labels: { team: 'support' },
    };

    await client.createFileWorkspace(createRequest);
    await client.getFileWorkspace('workspace-1');
    const files = await client.listFileWorkspaceFiles('workspace-1');
    await client.uploadFileWorkspaceFile('workspace-1', {
      fileName: 'uploaded.csv',
      mediaType: 'text/csv',
      content: 'id,value\n1,ok\n',
      provenance: { source: 'test' },
    });
    const blob = await client.downloadFileWorkspaceFileContent(files.files[0]!);
    await client.deleteFileWorkspaceFile('workspace-1', 'file-1');

    expect(fetchImpl.mock.calls[0]?.[1]?.body).toBe(JSON.stringify(createRequest));
    const uploadCall = fetchImpl.mock.calls.find((call) =>
      String(call[0]).endsWith('/api/v1/file-workspaces/workspace-1/files') && call[1]?.method === 'POST');
    const uploadHeaders = uploadCall?.[1]?.headers as Headers;
    expect(uploadHeaders.get('x-bpane-file-name')).toBe('uploaded.csv');
    expect(uploadHeaders.get('x-bpane-file-provenance')).toBe(JSON.stringify({ source: 'test' }));
    expect(uploadHeaders.get('content-type')).toBe('text/csv');
    expect(await blob.text()).toBe('downloaded content');
    expect(fetchImpl.mock.calls.at(-1)?.[0]).toEqual(
      new URL('http://browserpane.test/api/v1/file-workspaces/workspace-1/files/file-1'),
    );
  });

  it('loads project binding options from the project catalog', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () =>
      jsonResponse({
        projects: [{ id: 'project-1', name: 'Support', state: 'active', ignored: true }],
      }, 200));
    const client = new FileWorkspaceCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'token-1',
      fetchImpl,
    });

    const response = await client.listProjectOptions();

    expect(response.projects).toEqual([{ id: 'project-1', name: 'Support', state: 'active' }]);
    expect(fetchImpl.mock.calls[0]?.[0]).toEqual(new URL('http://browserpane.test/api/v1/projects'));
  });

  it('rejects invalid list payloads', () => {
    expect(() => toFileWorkspaceListResponse({ workspaces: {} })).toThrow(FileWorkspaceCatalogError);
  });
});

function jsonResponse(payload: unknown, status: number): Response {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

function workspacePayload() {
  return {
    id: 'workspace-1',
    project_id: 'project-1',
    project: { id: 'project-1', name: 'Support', state: 'active' },
    name: 'Support inputs',
    description: 'Reusable customer data',
    labels: { team: 'support' },
    files_path: '/api/v1/file-workspaces/workspace-1/files',
    created_at: '2026-06-20T09:00:00.000Z',
    updated_at: '2026-06-20T10:00:00.000Z',
  };
}

function filePayload(overrides: Partial<{ readonly name: string }> = {}) {
  return {
    id: 'file-1',
    workspace_id: 'workspace-1',
    name: overrides.name ?? 'fixture.csv',
    media_type: 'text/csv',
    byte_count: 18,
    sha256_hex: '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
    provenance: { source: 'test' },
    content_path: '/api/v1/file-workspaces/workspace-1/files/file-1/content',
    created_at: '2026-06-20T09:30:00.000Z',
    updated_at: '2026-06-20T09:30:00.000Z',
  };
}
