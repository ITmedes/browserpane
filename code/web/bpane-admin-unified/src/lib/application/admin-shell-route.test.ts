import { describe, expect, it } from 'vitest';
import { resolveAdminShellRoute } from './admin-shell-route';

describe('admin shell route metadata', () => {
  it.each([
    ['/admin-new/', 'dashboard', 'BrowserPane Dashboard'],
    ['/admin-new/projects', 'projects', 'BrowserPane Projects'],
    ['/admin-new/projects/new', 'projects', 'BrowserPane New Project'],
    ['/projects/project-1', 'projects', 'BrowserPane Project Details'],
    ['/admin-new/browser-contexts/context-1', 'contexts', 'BrowserPane Browser Context Details'],
    ['/admin-new/egress/new', 'egress', 'BrowserPane New Egress Profile'],
    ['/admin-new/files/workspaces/workspace-1', 'workspaces', 'BrowserPane File Workspace Details'],
    ['/admin-new/extensions', 'extensions', 'BrowserPane Approved Extensions'],
    ['/admin-new/extensions/new', 'extensions', 'BrowserPane New Approved Extension'],
    ['/admin-new/extensions/extension-1', 'extensions', 'BrowserPane Extension Details'],
    ['/admin-new/credential-bindings', 'credentials', 'BrowserPane Credential Bindings'],
    ['/admin-new/credential-bindings/new', 'credentials', 'BrowserPane New Credential Binding'],
    ['/admin-new/credential-bindings/binding-1', 'credentials', 'BrowserPane Credential Binding Details'],
    ['/admin-new/workflows/workflow-1', 'workflows', 'BrowserPane Workflow Details'],
    ['/admin-new/runs', 'runs', 'BrowserPane Workflow Runs'],
    ['/admin-new/runs/run-1', 'runs', 'BrowserPane Workflow Run Details'],
    ['/admin-new/workflow-runs', 'runs', 'BrowserPane Workflow Runs'],
    ['/admin-new/workflow-runs/run-1', 'runs', 'BrowserPane Workflow Run Details'],
    ['/admin-new/sessions/new', 'sessions', 'BrowserPane New Session'],
    ['/admin-new/sessions/session-1', 'sessions', 'BrowserPane Session Details'],
    ['/admin-new/sessions/session-1/live', 'sessions', 'BrowserPane Live Session'],
    ['/admin-new/sessions/session-1/files', 'sessions', 'BrowserPane Session Files'],
    ['/admin-new/sessions/session-1/recordings', 'sessions', 'BrowserPane Session Recordings'],
    ['/admin-new/sessions/session-1/network', 'sessions', 'BrowserPane Session Network'],
    ['/admin-new/recordings', 'recordings', 'BrowserPane Recordings'],
    ['/admin-new/identity', 'identity', 'BrowserPane Identity And Access'],
    ['/admin-new/workflow-event-subscriptions', 'events', 'BrowserPane Workflow Event Subscriptions'],
    ['/admin-new/workflow-event-subscriptions/new', 'events', 'BrowserPane New Workflow Event Subscription'],
    ['/admin-new/workflow-event-subscriptions/subscription-1', 'events', 'BrowserPane Workflow Event Subscription Details'],
    ['/admin-new/api', 'api', 'BrowserPane API Reference'],
    ['/admin-new/docs', 'memo', 'BrowserPane Integration Guide'],
    ['/admin-new/coverage', 'coverage', 'BrowserPane API Coverage'],
  ])('resolves %s', (pathname, activeId, title) => {
    expect(resolveAdminShellRoute(pathname)).toEqual({ activeId, title });
  });

  it('keeps the standalone session preview outside the admin shell', () => {
    expect(resolveAdminShellRoute('/admin-new/sessions/session-1/preview')).toBeNull();
  });
});
