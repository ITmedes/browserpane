import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import WorkflowDefinitionDetailRoute from './WorkflowDefinitionDetailRoute.svelte';

beforeEach(() => {
  window.history.replaceState(null, '', 'http://localhost:3000/admin-new/workflows/workflow-1');
});

afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});

describe('WorkflowDefinitionDetailRoute', () => {
  it('loads workflow detail and version metadata through the authenticated API', async () => {
    const previewWindow = { focus: vi.fn() } as unknown as Window;
    const openSpy = vi.spyOn(window, 'open').mockReturnValue(previewWindow);
    const fetchImpl = vi.fn<typeof fetch>(async (input) => {
      const url = String(input);
      if (url.endsWith('/api/v1/workflows/workflow-1')) {
        return jsonResponse(workflowPayload(), 200);
      }
      if (url.endsWith('/api/v1/workflows/workflow-1/versions')) {
        return jsonResponse({ versions: [versionPayload()] }, 200);
      }
      if (url.endsWith('/api/v1/workflows/workflow-1/versions/v1/source-files')) {
        return jsonResponse(sourceFilesPayload(), 200);
      }
      if (
        url.endsWith(
          '/api/v1/workflows/workflow-1/versions/v1/source-preview?path=dev%2Fworkflows%2Fbrowserpane-tour%2Frun.mjs',
        )
      ) {
        return jsonResponse(sourcePreviewPayload(), 200);
      }
      if (
        url.endsWith(
          '/api/v1/workflows/workflow-1/versions/v1/source-preview?path=dev%2Fworkflows%2Fbrowserpane-tour%2Fhelper.ts',
        )
      ) {
        return jsonResponse(
          sourcePreviewPayload({
            path: 'dev/workflows/browserpane-tour/helper.ts',
            content: 'export const helperValue = 1;',
          }),
          200,
        );
      }
      if (url.endsWith('/api/v1/workflow-runs')) {
        return jsonResponse(workflowRunPayload(), 201);
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(WorkflowDefinitionDetailRoute, {
      authContext: authContext({ accessTokenProvider: async () => 'shell-token' }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'workflow-definition-inspector').textContent).toContain('BrowserPane Tour');
    });
    expect(byTestId(target, 'workflow-definition-source').textContent).toContain('/workspace');
    await vi.waitFor(() => {
      expect(byTestId(target, 'workflow-code-preview-code').textContent).toContain('export default async function run');
    });
    expect(byTestId(target, 'workflow-code-preview-code-language').className).toContain('language-typescript');
    expect(byTestId(target, 'workflow-code-file-list').textContent).toContain('helper.ts');
    (target.querySelector('[data-source-path="dev/workflows/browserpane-tour/helper.ts"]') as HTMLButtonElement).click();
    await vi.waitFor(() => {
      expect(byTestId(target, 'workflow-code-preview-code').textContent).toContain('helperValue');
    });
    byTestId(target, 'workflow-run-start-connect').click();
    await vi.waitFor(() => {
      expect(byTestId(target, 'workflow-run-launch-success').textContent).toContain('run-1');
    });
    const createRunCall = fetchImpl.mock.calls.find((call) =>
      String(call[0]).endsWith('/api/v1/workflow-runs') && call[1]?.method === 'POST');
    expect(JSON.parse(String(createRunCall?.[1]?.body))).toMatchObject({
      workflow_id: 'workflow-1',
      version: 'v1',
      session: { create_session: { labels: { origin: 'admin-unified-workflow-run' } } },
    });
    expect(openSpy).toHaveBeenCalledWith(
      '/admin-new/sessions/session-1/preview',
      'bpane-session-preview-session-1',
      'popup=yes,width=1440,height=960,resizable=yes,scrollbars=no',
    );
    const headers = fetchImpl.mock.calls[0]?.[1]?.headers as Headers;
    expect(headers.get('authorization')).toBe('Bearer shell-token');
  });

  it('validates source and creates a new workflow version from the detail view', async () => {
    let versionCreated = false;
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/api/v1/workflows/workflow-1')) {
        return jsonResponse(workflowPayload({ latest_version: versionCreated ? 'v2' : 'v1' }), 200);
      }
      if (url.endsWith('/api/v1/workflows/workflow-1/versions') && init?.method === 'GET') {
        return jsonResponse(
          { versions: versionCreated ? [versionPayload(), versionPayload({ version: 'v2', id: 'version-2' })] : [versionPayload()] },
          200,
        );
      }
      if (url.endsWith('/api/v1/workflows/workflow-1/source-validation') && init?.method === 'POST') {
        return jsonResponse(sourceValidationPayload(), 200);
      }
      if (url.endsWith('/api/v1/workflows/workflow-1/versions') && init?.method === 'POST') {
        expect(JSON.parse(String(init.body))).toMatchObject({
          version: 'v2',
          source: { resolved_commit: 'def456' },
        });
        versionCreated = true;
        return jsonResponse(versionPayload({ version: 'v2', id: 'version-2', resolved_commit: 'def456' }), 201);
      }
      if (url.endsWith('/api/v1/workflows/workflow-1/versions/v1/source-files')) {
        return jsonResponse(sourceFilesPayload(), 200);
      }
      if (url.endsWith('/api/v1/workflows/workflow-1/versions/v2/source-files')) {
        return jsonResponse(sourceFilesPayload({ workflow_version: 'v2', resolved_commit: 'def456' }), 200);
      }
      if (url.includes('/api/v1/workflows/workflow-1/versions/v1/source-preview')) {
        return jsonResponse(sourcePreviewPayload(), 200);
      }
      if (url.includes('/api/v1/workflows/workflow-1/versions/v2/source-preview')) {
        return jsonResponse(sourcePreviewPayload({ workflow_version: 'v2', resolved_commit: 'def456' }), 200);
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(WorkflowDefinitionDetailRoute, {
      authContext: authContext({ accessTokenProvider: async () => 'shell-token' }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'workflow-source-editor').textContent).toContain('New version source');
    });

    byTestId(target, 'workflow-source-validate').click();

    await vi.waitFor(() => {
      expect(byTestId(target, 'workflow-source-validation-ready').textContent).toContain('def456');
    });
    byTestId(target, 'workflow-source-create-version').click();

    await vi.waitFor(() => {
      expect(byTestId(target, 'workflow-definition-action-success').textContent).toContain('v2');
    });
    await vi.waitFor(() => {
      expect(byTestId(target, 'workflow-definition-selected-version').textContent).toContain('v2');
    });
  });

  it('delegates authentication failures back to the shell', async () => {
    const onAuthenticationFailure = vi.fn();
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async () => new Response('unauthorized', { status: 401 })));
    const target = renderComponent(WorkflowDefinitionDetailRoute, {
      authContext: authContext({
        accessTokenProvider: async () => 'expired-token',
        onAuthenticationFailure,
      }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'workflow-definition-detail-error').textContent).toContain('Workflow definition detail unavailable');
    });
    expect(onAuthenticationFailure).toHaveBeenCalled();
  });
});

function authContext(
  overrides: Partial<Pick<UnifiedAdminContext, 'accessTokenProvider' | 'onAuthenticationFailure'>> = {},
): UnifiedAdminContext {
  return {
    auth: {
      configured: true,
      authenticated: true,
      username: 'demo',
      accessToken: 'token',
      claims: null,
    },
    authConfig: null,
    accessTokenProvider: overrides.accessTokenProvider ?? (async () => 'token'),
    onAuthenticationFailure: overrides.onAuthenticationFailure ?? vi.fn(),
    login: async () => {},
    logout: async () => {},
  };
}

function jsonResponse(payload: unknown, status: number): Response {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

function workflowPayload(overrides: Partial<{ readonly latest_version: string }> = {}) {
  return {
    id: 'workflow-1',
    name: 'BrowserPane Tour',
    description: 'Example tour',
    labels: { bpane_admin_template: 'browserpane-tour' },
    latest_version: overrides.latest_version ?? 'v1',
    created_at: '2026-06-21T09:00:00.000Z',
    updated_at: '2026-06-21T10:00:00.000Z',
  };
}

function versionPayload(
  overrides: Partial<{
    readonly id: string;
    readonly version: string;
    readonly resolved_commit: string;
  }> = {},
) {
  return {
    id: overrides.id ?? 'version-1',
    workflow_definition_id: 'workflow-1',
    version: overrides.version ?? 'v1',
    executor: 'playwright',
    entrypoint: 'dev/workflows/browserpane-tour/run.mjs',
    source: {
      kind: 'git',
      repository_url: '/workspace',
      ref: 'HEAD',
      resolved_commit: overrides.resolved_commit ?? 'abc123',
      root_path: 'dev',
    },
    input_schema: { type: 'object' },
    output_schema: null,
    default_session: null,
    allowed_credential_binding_ids: [],
    allowed_extension_ids: [],
    allowed_file_workspace_ids: [],
    created_at: '2026-06-21T09:30:00.000Z',
  };
}

function sourcePreviewPayload(
  overrides: Partial<{
    readonly workflow_version: string;
    readonly path: string;
    readonly content: string;
    readonly resolved_commit: string;
  }> = {},
) {
  return {
    workflow_definition_id: 'workflow-1',
    workflow_version: overrides.workflow_version ?? 'v1',
    entrypoint: 'dev/workflows/browserpane-tour/run.mjs',
    path: overrides.path ?? 'dev/workflows/browserpane-tour/run.mjs',
    source: {
      kind: 'git',
      repository_url: '/workspace',
      ref: 'HEAD',
      resolved_commit: overrides.resolved_commit ?? 'abc123',
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

function sourceFilesPayload(
  overrides: Partial<{
    readonly workflow_version: string;
    readonly resolved_commit: string;
  }> = {},
) {
  return {
    workflow_definition_id: 'workflow-1',
    workflow_version: overrides.workflow_version ?? 'v1',
    entrypoint: 'dev/workflows/browserpane-tour/run.mjs',
    source: {
      kind: 'git',
      repository_url: '/workspace',
      ref: 'HEAD',
      resolved_commit: overrides.resolved_commit ?? 'abc123',
      root_path: 'dev',
    },
    files: [
      {
        path: 'dev/workflows/browserpane-tour/run.mjs',
        byte_count: 38,
        media_type: 'text/javascript; charset=utf-8',
        language: 'typescript',
        entrypoint: true,
      },
      {
        path: 'dev/workflows/browserpane-tour/helper.ts',
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
    entrypoint: 'dev/workflows/browserpane-tour/run.mjs',
    source: {
      kind: 'git',
      repository_url: '/workspace',
      ref: 'HEAD',
      resolved_commit: 'def456',
      root_path: 'dev',
    },
    files: [
      {
        path: 'dev/workflows/browserpane-tour/run.mjs',
        byte_count: 38,
        media_type: 'text/javascript; charset=utf-8',
        language: 'typescript',
        entrypoint: true,
      },
    ],
  };
}

function workflowRunPayload() {
  return {
    id: 'run-1',
    workflow_definition_id: 'workflow-1',
    workflow_definition_version_id: 'workflow-version-1',
    workflow_version: 'v1',
    project_id: null,
    project: null,
    source_system: 'admin_unified',
    source_reference: 'workflow-detail',
    client_request_id: null,
    state: 'pending',
    session_id: 'session-1',
    automation_task_id: 'task-1',
    input: {},
    output: null,
    error: null,
    artifact_refs: [],
    produced_files: [],
    project_admission: null,
    admission: null,
    intervention: {},
    runtime: null,
    labels: {},
    started_at: null,
    completed_at: null,
    events_path: '/api/v1/workflow-runs/run-1/events',
    logs_path: '/api/v1/workflow-runs/run-1/logs',
    created_at: '2026-06-21T10:00:00.000Z',
    updated_at: '2026-06-21T10:00:00.000Z',
  };
}
