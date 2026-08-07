import { afterEach, describe, expect, it, vi } from 'vitest';

import { FileWorkspaceCatalogClient } from '$lib/file-workspaces/file-workspace-client';
import { SessionFileClient } from '$lib/session-files/session-file-client';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import SessionFileBindingsPanel from './SessionFileBindingsPanel.svelte';

afterEach(async () => {
  vi.restoreAllMocks();
  await cleanupRenderedComponents();
});

describe('SessionFileBindingsPanel', () => {
  it('creates, downloads, and removes an eligible workspace binding', async () => {
    let bindings: Record<string, unknown>[] = [];
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/api/v1/sessions/session-1/file-bindings') && init?.method === 'POST') {
        const request = JSON.parse(String(init.body));
        const created = bindingPayload({ mount_path: request.mount_path, mode: request.mode });
        bindings = [created];
        return jsonResponse(created);
      }
      if (url.endsWith('/api/v1/sessions/session-1/file-bindings')) {
        return jsonResponse({ bindings });
      }
      if (url.endsWith('/api/v1/file-workspaces')) {
        return jsonResponse({ workspaces: [workspacePayload()] });
      }
      if (url.endsWith('/api/v1/file-workspaces/workspace-1/files')) {
        return jsonResponse({ files: [workspaceFilePayload()] });
      }
      if (url.endsWith('/file-bindings/binding-1/content')) {
        return new Response('bound bytes', { status: 200 });
      }
      if (url.endsWith('/file-bindings/binding-1') && init?.method === 'DELETE') {
        bindings = [];
        return jsonResponse(bindingPayload({ state: 'removed' }));
      }
      return new Response('not found', { status: 404 });
    });
    vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:binding');
    vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {});
    vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
    const target = renderPanel(fetchImpl, { mutationAllowed: true });

    await vi.waitFor(() => {
      expect((byTestId(target, 'session-file-binding-create') as HTMLButtonElement).disabled).toBe(false);
    });
    expect((byTestId(target, 'session-file-binding-workspace') as HTMLSelectElement).value).toBe('workspace-1');
    expect((byTestId(target, 'session-file-binding-file') as HTMLSelectElement).value).toBe('workspace-file-1');
    expect((byTestId(target, 'session-file-binding-mount-path') as HTMLInputElement).value)
      .toBe('inputs/report.pdf');

    byTestId(target, 'session-file-binding-create').click();
    await vi.waitFor(() => {
      expect(byTestId(target, 'session-file-binding-mount').textContent).toContain('inputs/report.pdf');
    });
    expect(byTestId(target, 'session-file-bindings-action-success').textContent).toContain('Bound report.pdf');

    byTestId(target, 'session-file-binding-download').click();
    await vi.waitFor(() => {
      expect(byTestId(target, 'session-file-bindings-action-success').textContent).toContain('Download started');
    });

    byTestId(target, 'session-file-binding-remove').click();
    await vi.waitFor(() => {
      expect(byTestId(target, 'session-file-bindings-empty')).toBeTruthy();
    });
    const createCall = fetchImpl.mock.calls.find((call) => call[1]?.method === 'POST');
    expect(JSON.parse(String(createCall?.[1]?.body))).toEqual({
      workspace_id: 'workspace-1',
      file_id: 'workspace-file-1',
      mount_path: 'inputs/report.pdf',
      mode: 'read_only',
      labels: { source: 'admin-new' },
    });
  });

  it('blocks mutation while preserving existing binding evidence', async () => {
    const target = renderPanel(async (input) => {
      const url = String(input);
      if (url.endsWith('/file-bindings')) {
        return jsonResponse({ bindings: [bindingPayload()] });
      }
      if (url.endsWith('/file-workspaces')) {
        return jsonResponse({ workspaces: [workspacePayload()] });
      }
      if (url.endsWith('/workspace-1/files')) {
        return jsonResponse({ files: [workspaceFilePayload()] });
      }
      return new Response('not found', { status: 404 });
    }, {
      mutationAllowed: false,
      policyMessage: 'Project Support blocks session file bindings.',
    });

    await vi.waitFor(() => expect(byTestId(target, 'session-file-binding-row')).toBeTruthy());
    expect(byTestId(target, 'session-file-bindings-policy-blocked').textContent).toContain('Project Support');
    expect((byTestId(target, 'session-file-binding-create') as HTMLButtonElement).disabled).toBe(true);
    expect((byTestId(target, 'session-file-binding-remove') as HTMLButtonElement).disabled).toBe(true);
  });

  it('keeps binding and workspace catalog failures section-local', async () => {
    const target = renderPanel(async (input) => {
      const url = String(input);
      if (url.endsWith('/file-bindings')) {
        return new Response('bindings down', { status: 503 });
      }
      return new Response('workspaces down', { status: 502 });
    }, { mutationAllowed: true });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-file-bindings-error').textContent).toContain('HTTP 503');
    });
    expect(byTestId(target, 'session-file-bindings-workspace-error').textContent).toContain('HTTP 502');
  });
});

function renderPanel(
  fetchImpl: typeof fetch,
  props: { readonly mutationAllowed: boolean; readonly policyMessage?: string } = { mutationAllowed: true },
) {
  const options = {
    baseUrl: 'http://localhost:8080',
    accessTokenProvider: async () => 'token',
    fetchImpl,
  };
  return renderComponent(SessionFileBindingsPanel, {
    sessionClient: new SessionFileClient(options),
    workspaceClient: new FileWorkspaceCatalogClient(options),
    sessionId: 'session-1',
    projectId: 'project-1',
    mutationAllowed: props.mutationAllowed,
    policyMessage: props.policyMessage ?? null,
    allowedWorkspaceIds: ['workspace-1'],
  });
}

function workspacePayload() {
  return {
    id: 'workspace-1',
    project_id: 'project-1',
    project: { id: 'project-1', name: 'Support', state: 'active' },
    name: 'Support inputs',
    description: null,
    labels: {},
    files_path: '/api/v1/file-workspaces/workspace-1/files',
    created_at: '2026-08-07T10:00:00Z',
    updated_at: '2026-08-07T10:00:00Z',
  };
}

function workspaceFilePayload() {
  return {
    id: 'workspace-file-1',
    workspace_id: 'workspace-1',
    name: 'report.pdf',
    media_type: 'application/pdf',
    byte_count: 2048,
    sha256_hex: '1234567890abcdef1234',
    provenance: null,
    content_path: '/api/v1/file-workspaces/workspace-1/files/workspace-file-1/content',
    created_at: '2026-08-07T10:00:00Z',
    updated_at: '2026-08-07T10:00:00Z',
  };
}

function bindingPayload(overrides: Readonly<Record<string, unknown>> = {}) {
  return {
    id: 'binding-1',
    session_id: 'session-1',
    workspace_id: 'workspace-1',
    file_id: 'workspace-file-1',
    file_name: 'report.pdf',
    media_type: 'application/pdf',
    byte_count: 2048,
    sha256_hex: '1234567890abcdef1234',
    provenance: null,
    mount_path: 'inputs/report.pdf',
    mode: 'read_only',
    state: 'materialized',
    error: null,
    labels: {},
    content_path: '/api/v1/sessions/session-1/file-bindings/binding-1/content',
    created_at: '2026-08-07T10:00:00Z',
    updated_at: '2026-08-07T10:00:00Z',
    ...overrides,
  };
}

function jsonResponse(payload: unknown): Response {
  return new Response(JSON.stringify(payload), {
    status: 200,
    headers: { 'content-type': 'application/json' },
  });
}
