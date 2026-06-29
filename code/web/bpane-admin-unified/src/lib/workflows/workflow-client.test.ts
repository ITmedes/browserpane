import { describe, expect, it, vi } from 'vitest';

import {
  WorkflowCatalogClient,
  WorkflowCatalogError,
  toWorkflowDefinitionListResponse,
  toWorkflowDefinitionSourceFileListResponse,
  toWorkflowDefinitionSourceValidationResponse,
  toWorkflowDefinitionSourcePreviewResource,
} from './workflow-client';

describe('WorkflowCatalogClient', () => {
  it('loads workflow definitions through the authenticated API', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () => jsonResponse({ workflows: [workflowPayload()] }, 200));
    const client = new WorkflowCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'token-1',
      fetchImpl,
    });

    const response = await client.listDefinitions();

    expect(response.workflows[0]).toMatchObject({
      id: 'workflow-1',
      name: 'Support tour',
      latest_version: 'v1',
    });
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL('http://browserpane.test/api/v1/workflows'),
      expect.objectContaining({ method: 'GET' }),
    );
    const headers = fetchImpl.mock.calls[0]?.[1]?.headers as Headers;
    expect(headers.get('authorization')).toBe('Bearer token-1');
  });

  it('creates definitions and versions and encodes identifiers', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/api/v1/workflows') && init?.method === 'POST') {
        return jsonResponse(workflowPayload(), 201);
      }
      if (url.endsWith('/api/v1/workflows/workflow%2Fwith%2Fslash/versions') && init?.method === 'POST') {
        return jsonResponse(versionPayload(), 201);
      }
      if (url.endsWith('/api/v1/workflows/workflow%2Fwith%2Fslash/versions/v1%2Fcandidate')) {
        return jsonResponse(versionPayload({ version: 'v1/candidate' }), 200);
      }
      return new Response('not found', { status: 404 });
    });
    const client = new WorkflowCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'token-1',
      fetchImpl,
    });
    const definitionRequest = {
      name: 'Support tour',
      description: 'Walk through support pages',
      labels: { team: 'support' },
    };
    const versionRequest = {
      version: 'v1',
      executor: 'playwright',
      entrypoint: 'dev/workflows/support/run.mjs',
      source: { kind: 'git', repository_url: '/workspace', ref: 'HEAD', root_path: 'dev' },
    };

    await client.createDefinition(definitionRequest);
    await client.createDefinitionVersion('workflow/with/slash', versionRequest);
    const version = await client.getDefinitionVersion('workflow/with/slash', 'v1/candidate');

    expect(fetchImpl.mock.calls[0]?.[1]?.body).toBe(JSON.stringify(definitionRequest));
    expect(fetchImpl.mock.calls[1]?.[0]).toEqual(
      new URL('http://browserpane.test/api/v1/workflows/workflow%2Fwith%2Fslash/versions'),
    );
    expect(fetchImpl.mock.calls[1]?.[1]?.body).toBe(JSON.stringify(versionRequest));
    expect(version.version).toBe('v1/candidate');
  });

  it('loads workflow definition source previews', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input) => {
      const url = String(input);
      if (
        url.endsWith(
          '/api/v1/workflows/workflow%2Fwith%2Fslash/versions/v1%2Fcandidate/source-preview?path=dev%2Fworkflows%2Fsupport%2Fhelper.ts',
        )
      ) {
        return jsonResponse(
          sourcePreviewPayload({
            workflow_version: 'v1/candidate',
            path: 'dev/workflows/support/helper.ts',
            content: 'export const helperValue = 1;',
          }),
          200,
        );
      }
      return new Response('not found', { status: 404 });
    });
    const client = new WorkflowCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'token-1',
      fetchImpl,
    });

    const preview = await client.getDefinitionSourcePreview(
      'workflow/with/slash',
      'v1/candidate',
      'dev/workflows/support/helper.ts',
    );

    expect(preview.workflow_version).toBe('v1/candidate');
    expect(preview.path).toBe('dev/workflows/support/helper.ts');
    expect(preview.language).toBe('typescript');
    expect(preview.content).toContain('helperValue');
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL(
        'http://browserpane.test/api/v1/workflows/workflow%2Fwith%2Fslash/versions/v1%2Fcandidate/source-preview?path=dev%2Fworkflows%2Fsupport%2Fhelper.ts',
      ),
      expect.objectContaining({ method: 'GET' }),
    );
  });

  it('loads workflow definition source file listings', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input) => {
      const url = String(input);
      if (url.endsWith('/api/v1/workflows/workflow%2Fwith%2Fslash/versions/v1%2Fcandidate/source-files')) {
        return jsonResponse(sourceFileListPayload({ workflow_version: 'v1/candidate' }), 200);
      }
      return new Response('not found', { status: 404 });
    });
    const client = new WorkflowCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'token-1',
      fetchImpl,
    });

    const listing = await client.listDefinitionSourceFiles('workflow/with/slash', 'v1/candidate');

    expect(listing.workflow_version).toBe('v1/candidate');
    expect(listing.files).toHaveLength(2);
    expect(listing.files[0]).toMatchObject({
      path: 'dev/workflows/support/run.mjs',
      entrypoint: true,
    });
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL('http://browserpane.test/api/v1/workflows/workflow%2Fwith%2Fslash/versions/v1%2Fcandidate/source-files'),
      expect.objectContaining({ method: 'GET' }),
    );
  });

  it('validates workflow definition sources without creating a version', async () => {
    const request = {
      entrypoint: 'dev/workflows/support/run.mjs',
      source: {
        kind: 'git' as const,
        repository_url: '/workspace',
        ref: 'HEAD',
        resolved_commit: null,
        root_path: 'dev',
      },
    };
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/api/v1/workflows/workflow%2Fwith%2Fslash/source-validation')) {
        expect(init?.body).toBe(JSON.stringify(request));
        return jsonResponse(sourceValidationPayload(), 200);
      }
      return new Response('not found', { status: 404 });
    });
    const client = new WorkflowCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'token-1',
      fetchImpl,
    });

    const validation = await client.validateDefinitionSource('workflow/with/slash', request);

    expect(validation.source.resolved_commit).toBe('abc123');
    expect(validation.files[0]).toMatchObject({
      path: 'dev/workflows/support/run.mjs',
      entrypoint: true,
    });
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL('http://browserpane.test/api/v1/workflows/workflow%2Fwith%2Fslash/source-validation'),
      expect.objectContaining({ method: 'POST' }),
    );
  });

  it('loads definition detail and version lists', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input) => {
      const url = String(input);
      if (url.endsWith('/api/v1/workflows/workflow-1')) {
        return jsonResponse(workflowPayload(), 200);
      }
      if (url.endsWith('/api/v1/workflows/workflow-1/versions')) {
        return jsonResponse({ versions: [versionPayload()] }, 200);
      }
      return new Response('not found', { status: 404 });
    });
    const client = new WorkflowCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'token-1',
      fetchImpl,
    });

    const definition = await client.getDefinition('workflow-1');
    const versions = await client.listDefinitionVersions('workflow-1');

    expect(definition.id).toBe('workflow-1');
    expect(versions.versions[0]?.source?.repository_url).toBe('/workspace');
  });

  it('delegates authentication failures', async () => {
    const onAuthenticationFailure = vi.fn();
    const client = new WorkflowCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'expired-token',
      fetchImpl: async () => new Response('unauthorized', { status: 401 }),
      onAuthenticationFailure,
    });

    await expect(client.listDefinitions()).rejects.toMatchObject({
      code: 'http_error',
      status: 401,
    });
    expect(onAuthenticationFailure).toHaveBeenCalledOnce();
  });

  it('rejects invalid list payloads', () => {
    expect(() => toWorkflowDefinitionListResponse({ workflows: {} })).toThrow(WorkflowCatalogError);
  });

  it('rejects invalid source preview payloads', () => {
    expect(() => toWorkflowDefinitionSourcePreviewResource({ content: 42 })).toThrow(WorkflowCatalogError);
  });

  it('rejects invalid source file listing payloads', () => {
    expect(() => toWorkflowDefinitionSourceFileListResponse({ files: [{}] })).toThrow(WorkflowCatalogError);
  });

  it('rejects invalid source validation payloads', () => {
    expect(() => toWorkflowDefinitionSourceValidationResponse({ source: null })).toThrow(WorkflowCatalogError);
  });
});

function jsonResponse(payload: unknown, status: number): Response {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

function workflowPayload() {
  return {
    id: 'workflow-1',
    name: 'Support tour',
    description: 'Walk through support pages',
    labels: { team: 'support' },
    latest_version: 'v1',
    created_at: '2026-06-21T09:00:00.000Z',
    updated_at: '2026-06-21T10:00:00.000Z',
  };
}

function versionPayload(overrides: Partial<{ readonly version: string }> = {}) {
  return {
    id: 'version-1',
    workflow_definition_id: 'workflow-1',
    version: overrides.version ?? 'v1',
    executor: 'playwright',
    entrypoint: 'dev/workflows/support/run.mjs',
    source: {
      kind: 'git',
      repository_url: '/workspace',
      ref: 'HEAD',
      resolved_commit: 'abc123',
      root_path: 'dev',
    },
    input_schema: { type: 'object' },
    output_schema: null,
    default_session: { labels: { purpose: 'support' } },
    allowed_credential_binding_ids: ['credential-1'],
    allowed_extension_ids: [],
    allowed_file_workspace_ids: ['workspace-1'],
    created_at: '2026-06-21T09:30:00.000Z',
  };
}

function sourcePreviewPayload(
  overrides: Partial<{
    readonly workflow_version: string;
    readonly path: string;
    readonly content: string;
  }> = {},
) {
  return {
    workflow_definition_id: 'workflow-1',
    workflow_version: overrides.workflow_version ?? 'v1',
    entrypoint: 'dev/workflows/support/run.mjs',
    path: overrides.path ?? 'dev/workflows/support/run.mjs',
    source: {
      kind: 'git',
      repository_url: '/workspace',
      ref: 'HEAD',
      resolved_commit: 'abc123',
      root_path: 'dev',
    },
    media_type: 'text/javascript; charset=utf-8',
    language: 'typescript',
    content: overrides.content ?? 'export default async function run() {}',
    byte_count: 38,
    max_bytes: 65536,
    truncated: false,
  };
}

function sourceFileListPayload(overrides: Partial<{ readonly workflow_version: string }> = {}) {
  return {
    workflow_definition_id: 'workflow-1',
    workflow_version: overrides.workflow_version ?? 'v1',
    entrypoint: 'dev/workflows/support/run.mjs',
    source: {
      kind: 'git',
      repository_url: '/workspace',
      ref: 'HEAD',
      resolved_commit: 'abc123',
      root_path: 'dev',
    },
    files: [
      {
        path: 'dev/workflows/support/run.mjs',
        byte_count: 38,
        media_type: 'text/javascript; charset=utf-8',
        language: 'typescript',
        entrypoint: true,
      },
      {
        path: 'dev/workflows/support/helper.ts',
        byte_count: 29,
        media_type: 'text/typescript; charset=utf-8',
        language: 'typescript',
        entrypoint: false,
      },
    ],
  };
}

function sourceValidationPayload() {
  return {
    workflow_definition_id: 'workflow-1',
    entrypoint: 'dev/workflows/support/run.mjs',
    source: {
      kind: 'git',
      repository_url: '/workspace',
      ref: 'HEAD',
      resolved_commit: 'abc123',
      root_path: 'dev',
    },
    files: [
      {
        path: 'dev/workflows/support/run.mjs',
        byte_count: 38,
        media_type: 'text/javascript; charset=utf-8',
        language: 'typescript',
        entrypoint: true,
      },
    ],
  };
}
