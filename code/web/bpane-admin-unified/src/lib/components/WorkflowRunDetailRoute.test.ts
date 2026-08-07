import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import {
  workflowRunEventFixture,
  workflowRunFixture,
  workflowRunLogFixture,
  workflowRunProducedFileFixture,
} from '$lib/test-utils/workflow-run-fixture';
import WorkflowRunDetailRoute from './WorkflowRunDetailRoute.svelte';

afterEach(async () => {
  vi.unstubAllGlobals();
  window.history.replaceState({}, '', '/');
  await cleanupRenderedComponents();
});

describe('WorkflowRunDetailRoute', () => {
  it('loads a canonical deep link with authenticated independent evidence requests', async () => {
    window.history.replaceState({}, '', '/admin-new/workflow-runs/run-1');
    const fetchImpl = vi.fn<typeof fetch>(workflowRunFetch());
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(WorkflowRunDetailRoute, { authContext: authContext() });

    await vi.waitFor(() => {
      expect(byTestId(target, 'workflow-run-detail-title').textContent).toContain('run-1');
      expect(byTestId(target, 'workflow-run-detail-event-count').textContent).toContain('1');
      expect(byTestId(target, 'workflow-run-detail-log-count').textContent).toContain('1');
      expect(byTestId(target, 'workflow-run-detail-produced-file-count').textContent).toContain('1');
    });
    expect(fetchImpl).toHaveBeenCalledTimes(4);
    for (const [, init] of fetchImpl.mock.calls) {
      expect((init?.headers as Headers).get('authorization')).toBe('Bearer shell-token');
    }
  });

  it('submits operator input and replaces the run with the authoritative response', async () => {
    window.history.replaceState({}, '', '/admin-new/runs/run-1');
    let run = workflowRunFixture();
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/submit-input') && init?.method === 'POST') {
        expect(JSON.parse(String(init.body))).toMatchObject({ input: { approved: true } });
        run = workflowRunFixture({
          state: 'running',
          intervention: { pending_request: null, last_resolution: null },
        });
        return jsonResponse(run);
      }
      return workflowRunFetch(() => run)(input, init);
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(WorkflowRunDetailRoute, { authContext: authContext() });

    await vi.waitFor(() => {
      expect(byTestId(target, 'workflow-run-detail-state').textContent).toBe('awaiting_input');
    });
    setInputValue(byTestId(target, 'workflow-run-detail-operator-input'), '{"approved":true}');
    await tick();
    byTestId(target, 'workflow-run-detail-submit-input').click();

    await vi.waitFor(() => {
      expect(byTestId(target, 'workflow-run-detail-state').textContent).toBe('running');
      expect(byTestId(target, 'workflow-run-detail-action-success').textContent)
        .toContain('Operator input submitted');
    });
  });

  it('keeps partial evidence failures local and delegates 401 recovery', async () => {
    window.history.replaceState({}, '', '/admin-new/workflow-runs/run-1');
    const onAuthenticationFailure = vi.fn();
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async (input) => {
      const url = String(input);
      if (url.endsWith('/api/v1/workflow-runs/run-1')) {
        return jsonResponse(workflowRunFixture());
      }
      if (url.endsWith('/events')) {
        return new Response(JSON.stringify({ error: 'token expired' }), { status: 401 });
      }
      if (url.endsWith('/logs')) {
        return jsonResponse({ logs: [workflowRunLogFixture()] });
      }
      return new Response(JSON.stringify({ error: 'artifact store unavailable' }), { status: 503 });
    }));
    const target = renderComponent(WorkflowRunDetailRoute, {
      authContext: authContext({ onAuthenticationFailure }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'workflow-run-detail-events-error').textContent).toContain('HTTP 401');
      expect(byTestId(target, 'workflow-run-detail-log').textContent).toContain('Waiting for operator');
      expect(byTestId(target, 'workflow-run-detail-produced-files-error').textContent).toContain('HTTP 503');
    });
    expect(onAuthenticationFailure).toHaveBeenCalledOnce();
  });

  it('shows a route-local error for a missing run identifier', async () => {
    window.history.replaceState({}, '', '/admin-new/workflow-runs');
    const target = renderComponent(WorkflowRunDetailRoute, { authContext: authContext() });

    await vi.waitFor(() => {
      expect(byTestId(target, 'workflow-run-detail-error').textContent).toContain('missing or invalid');
    });
  });
});

function workflowRunFetch(
  runProvider: () => ReturnType<typeof workflowRunFixture> = workflowRunFixture,
): typeof fetch {
  return async (input) => {
    const url = String(input);
    if (url.endsWith('/api/v1/workflow-runs/run-1')) {
      return jsonResponse(runProvider());
    }
    if (url.endsWith('/events')) {
      return jsonResponse({ events: [workflowRunEventFixture()] });
    }
    if (url.endsWith('/logs')) {
      return jsonResponse({ logs: [workflowRunLogFixture()] });
    }
    if (url.endsWith('/produced-files')) {
      return jsonResponse({ files: [workflowRunProducedFileFixture()] });
    }
    return new Response('not found', { status: 404 });
  };
}

function authContext(
  overrides: Partial<Pick<UnifiedAdminContext, 'onAuthenticationFailure'>> = {},
): UnifiedAdminContext {
  return {
    auth: {
      configured: true,
      authenticated: true,
      username: 'demo',
      accessToken: 'shell-token',
      claims: null,
    },
    authConfig: null,
    accessTokenProvider: async () => 'shell-token',
    onAuthenticationFailure: overrides.onAuthenticationFailure ?? vi.fn(),
    login: async () => {},
    logout: async () => {},
  };
}

function jsonResponse(payload: unknown): Response {
  return new Response(JSON.stringify(payload), {
    status: 200,
    headers: { 'content-type': 'application/json' },
  });
}

function setInputValue(element: Element, value: string): void {
  (element as HTMLTextAreaElement).value = value;
  element.dispatchEvent(new Event('input', { bubbles: true }));
}
