import { tick } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import {
  projectDetailAuthContext as authContext,
  projectDetailJsonResponse as jsonResponse,
  projectDetailPayload as projectPayloadFor,
} from '$lib/test-utils/project-detail-route-support';
import { sessionPayload } from '$lib/test-utils/session-fixtures';
import { workflowRunFixture } from '$lib/test-utils/workflow-run-fixture';
import ProjectDetailRoute from './ProjectDetailRoute.svelte';

const PROJECT_ID = '11111111-1111-4111-8111-111111111111';
const projectPayload = () => projectPayloadFor(PROJECT_ID);

beforeEach(() => {
  window.history.replaceState(null, '', `http://localhost:3000/admin-new/projects/${PROJECT_ID}`);
});

afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});

describe('ProjectDetailRoute', () => {
  it('loads, refreshes usage, and saves an existing project through the authenticated API', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = input.toString();
      if (init?.method === 'PUT') {
        return jsonResponse({
          ...projectPayload(),
          description: 'Updated support browser work',
          labels: { team: 'support', env: 'prod' },
        }, 200);
      }
      if (url.endsWith('/usage')) {
        return jsonResponse({
          ...projectPayload().usage,
          session_creations: 2,
        }, 200);
      }
      if (url.endsWith('/api/v1/sessions')) {
        return jsonResponse({
          sessions: [{
            ...sessionPayload({ id: 'session-project' }),
            project_id: PROJECT_ID,
            project: { id: PROJECT_ID, name: 'Support', state: 'active' },
          }],
        }, 200);
      }
      if (url.endsWith('/api/v1/workflow-runs')) {
        return jsonResponse({
          runs: [workflowRunFixture({
            id: 'run-project',
            project_id: PROJECT_ID,
            project: { id: PROJECT_ID, name: 'Support', state: 'active' },
          })],
        }, 200);
      }
      if (url.endsWith('/session-templates')) {
        return jsonResponse({
          templates: [{
            id: 'template-support',
            name: 'Support Browser',
            description: 'Approved support defaults',
            labels: {},
            defaults: {},
            version: 1,
            created_at: '2026-06-11T09:00:00.000Z',
            updated_at: '2026-06-11T10:00:00.000Z',
          }],
        }, 200);
      }
      if (url.endsWith('/browser-contexts')) {
        return jsonResponse({
          contexts: [{
            id: '22222222-2222-4222-8222-222222222222',
            project_id: null,
            project: null,
            name: 'Support Context',
            description: null,
            labels: {},
            persistence_mode: 'reusable',
            retention_sec: null,
            retention_expires_at: null,
            max_profile_storage_bytes: null,
            state: 'ready',
            usage: {},
            created_at: '2026-06-11T09:00:00.000Z',
            updated_at: '2026-06-11T10:00:00.000Z',
            last_used_at: null,
            deleted_at: null,
          }],
        }, 200);
      }
      if (url.endsWith('/egress-profiles')) {
        return jsonResponse({ profiles: [] }, 200);
      }
      if (url.endsWith('/extensions')) {
        return jsonResponse({ extensions: [] }, 200);
      }
      if (url.endsWith('/file-workspaces')) {
        return jsonResponse({ workspaces: [] }, 200);
      }
      return jsonResponse(projectPayload(), 200);
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(ProjectDetailRoute, {
      authContext: authContext({ accessTokenProvider: async () => 'shell-token' }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'project-detail-route').textContent).toContain('Project settings');
      expect(byTestId(target, 'project-related-sessions').textContent).toContain('session-project');
      expect(byTestId(target, 'project-related-workflow-runs').textContent).toContain('run-project');
    });
    byTestId(target, 'project-refresh-usage').click();
    await vi.waitFor(() => {
      expect(byTestId(target, 'project-action-success').textContent).toContain('Project usage refreshed');
    });

    const description = byTestId(target, 'project-edit-description') as HTMLTextAreaElement;
    description.value = 'Updated support browser work';
    description.dispatchEvent(new Event('input', { bubbles: true }));
    const labels = byTestId(target, 'project-edit-labels') as HTMLTextAreaElement;
    labels.value = 'env=prod\nteam=support';
    labels.dispatchEvent(new Event('input', { bubbles: true }));
    await tick();
    byTestId(target, 'project-edit-save').click();

    await vi.waitFor(() => {
      expect(byTestId(target, 'project-action-success').textContent).toContain('Project saved');
    });
    const putCall = fetchImpl.mock.calls.find((call) => call[1]?.method === 'PUT');
    expect(putCall?.[0]).toEqual(new URL(`http://localhost:3000/api/v1/projects/${PROJECT_ID}`));
    expect(JSON.parse(putCall?.[1]?.body as string)).toMatchObject({
      name: 'Support',
      description: 'Updated support browser work',
      labels: { env: 'prod', team: 'support' },
      quotas: {},
      policy: expect.objectContaining({ allow_browser_uploads: true }),
      state: 'active',
    });
  });

  it('delegates authentication failures back to the shell', async () => {
    const onAuthenticationFailure = vi.fn();
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async () => new Response('unauthorized', { status: 401 })));
    const target = renderComponent(ProjectDetailRoute, {
      authContext: authContext({
        accessTokenProvider: async () => 'expired-token',
        onAuthenticationFailure,
      }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'project-detail-error').textContent).toContain('Project detail unavailable');
    });
    expect(onAuthenticationFailure).toHaveBeenCalled();
  });

  it('keeps project editing and successful work evidence available after a partial failure', async () => {
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async (input) => {
      const url = input.toString();
      if (url.endsWith('/api/v1/sessions')) {
        return new Response('session catalog unavailable', { status: 503 });
      }
      if (url.endsWith('/api/v1/workflow-runs')) {
        return jsonResponse({ runs: [workflowRunFixture({ project_id: PROJECT_ID })] }, 200);
      }
      return jsonResponse(projectPayload(), 200);
    }));
    const target = renderComponent(ProjectDetailRoute, { authContext: authContext() });

    await vi.waitFor(() => {
      expect(byTestId(target, 'project-edit-form').textContent).toContain('Project settings');
      expect(byTestId(target, 'project-related-sessions').textContent).toContain('unavailable');
      expect(byTestId(target, 'project-related-workflow-runs').textContent).toContain('run-1');
    });
  });
});
