import { tick } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import BrowserContextImportRoute from './BrowserContextImportRoute.svelte';

beforeEach(() => {
  window.history.replaceState(null, '', 'http://localhost:3000/admin-new/browser-contexts/import');
});

afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});

describe('BrowserContextImportRoute', () => {
  it('uploads a BrowserPane archive with editable target metadata', async () => {
    const navigateToContext = vi.fn();
    const archive = new File(['archive-bytes'], 'support-export.zip', { type: 'application/zip' });
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/api/v1/projects') && init?.method === 'GET') {
        return jsonResponse(projectListPayload(), 200);
      }
      if (url.endsWith('/api/v1/browser-contexts/import') && init?.method === 'POST') {
        return jsonResponse(browserContextPayload(), 201);
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(BrowserContextImportRoute, {
      authContext: authContext({ accessTokenProvider: async () => 'shell-token' }),
      navigateToContext,
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'browser-context-edit-form')).toBeInstanceOf(HTMLElement);
    });
    await input(target, 'browser-context-edit-name', 'Imported support baseline');
    await selectFile(target, archive);
    expect(byTestId(target, 'browser-context-import-file-summary').textContent).toContain(
      'support-export.zip',
    );
    byTestId(target, 'browser-context-edit-save').click();

    await vi.waitFor(() => {
      expect(navigateToContext).toHaveBeenCalledWith(expect.objectContaining({ id: 'context-imported' }));
    });
    const importCall = fetchImpl.mock.calls.find((call) =>
      String(call[0]).endsWith('/api/v1/browser-contexts/import') && call[1]?.method === 'POST');
    expect(importCall?.[1]?.body).toBe(archive);
    const headers = importCall?.[1]?.headers as Headers;
    expect(headers.get('authorization')).toBe('Bearer shell-token');
    expect(headers.get('content-type')).toBe('application/zip');
    expect(headers.get('x-bpane-browser-context-name')).toBe('Imported support baseline');
    expect(headers.get('x-bpane-browser-context-retention-sec')).toBe('604800');
    expect(headers.has('x-bpane-browser-context-project-id')).toBe(false);
  });

  it.each([400, 413, 429])('preserves the archive and draft after an HTTP %s rejection', async (status) => {
    const archive = new File(['archive-bytes'], 'retry-import.zip', { type: 'application/zip' });
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/api/v1/projects') && init?.method === 'GET') {
        return jsonResponse(projectListPayload(), 200);
      }
      if (url.endsWith('/api/v1/browser-contexts/import') && init?.method === 'POST') {
        return jsonResponse({ error: 'import rejected' }, status);
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(BrowserContextImportRoute, {
      authContext: authContext(),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'browser-context-edit-form')).toBeInstanceOf(HTMLElement);
    });
    await input(target, 'browser-context-edit-name', 'Retry import');
    await selectFile(target, archive);
    byTestId(target, 'browser-context-edit-save').click();

    await vi.waitFor(() => {
      expect(byTestId(target, 'browser-context-import-error').textContent).toContain(`HTTP ${status}`);
    });
    expect((byTestId(target, 'browser-context-edit-name') as HTMLInputElement).value).toBe('Retry import');
    expect(byTestId(target, 'browser-context-import-file-summary').textContent).toContain('retry-import.zip');
    expect((byTestId(target, 'browser-context-edit-save') as HTMLButtonElement).disabled).toBe(false);
  });
});

async function input(target: HTMLElement, testId: string, value: string): Promise<void> {
  const element = byTestId(target, testId) as HTMLInputElement;
  element.value = value;
  element.dispatchEvent(new Event('input', { bubbles: true }));
  await tick();
}

async function selectFile(target: HTMLElement, file: File): Promise<void> {
  const inputElement = byTestId(target, 'browser-context-import-file') as HTMLInputElement;
  Object.defineProperty(inputElement, 'files', {
    configurable: true,
    value: [file],
  });
  inputElement.dispatchEvent(new Event('change', { bubbles: true }));
  await tick();
}

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

function projectListPayload() {
  return {
    projects: [{ id: 'project-1', name: 'Support', state: 'active' }],
  };
}

function browserContextPayload() {
  return {
    id: 'context-imported',
    project_id: null,
    project: null,
    name: 'Imported support baseline',
    description: null,
    labels: {},
    persistence_mode: 'reusable',
    retention_sec: 604800,
    retention_expires_at: null,
    max_profile_storage_bytes: null,
    state: 'ready',
    usage: {
      visible_session_count: 0,
      active_runtime_session_count: 0,
      active_runtime_session_id: null,
      profile_storage_bytes: 13,
      profile_storage_limit_exceeded: false,
    },
    created_at: '2026-08-07T10:00:00.000Z',
    updated_at: '2026-08-07T10:00:00.000Z',
    last_used_at: null,
    deleted_at: null,
  };
}
