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
    const headers = fetchImpl.mock.calls[0]?.[1]?.headers as Headers;
    expect(headers.get('authorization')).toBe('Bearer shell-token');
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

function workflowPayload() {
  return {
    id: 'workflow-1',
    name: 'BrowserPane Tour',
    description: 'Example tour',
    labels: { bpane_admin_template: 'browserpane-tour' },
    latest_version: 'v1',
    created_at: '2026-06-21T09:00:00.000Z',
    updated_at: '2026-06-21T10:00:00.000Z',
  };
}

function versionPayload() {
  return {
    id: 'version-1',
    workflow_definition_id: 'workflow-1',
    version: 'v1',
    executor: 'playwright',
    entrypoint: 'dev/workflows/browserpane-tour/run.mjs',
    source: {
      kind: 'git',
      repository_url: '/workspace',
      ref: 'HEAD',
      resolved_commit: 'abc123',
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
    readonly path: string;
    readonly content: string;
  }> = {},
) {
  return {
    workflow_definition_id: 'workflow-1',
    workflow_version: 'v1',
    entrypoint: 'dev/workflows/browserpane-tour/run.mjs',
    path: overrides.path ?? 'dev/workflows/browserpane-tour/run.mjs',
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

function sourceFilesPayload() {
  return {
    workflow_definition_id: 'workflow-1',
    workflow_version: 'v1',
    entrypoint: 'dev/workflows/browserpane-tour/run.mjs',
    source: {
      kind: 'git',
      repository_url: '/workspace',
      ref: 'HEAD',
      resolved_commit: 'abc123',
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
