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
    ['/admin-new/workflows/workflow-1', 'workflows', 'BrowserPane Workflow Details'],
    ['/admin-new/runs', 'runs', 'BrowserPane Workflow Runs'],
    ['/admin-new/sessions/new', 'sessions', 'BrowserPane New Session'],
    ['/admin-new/sessions/session-1', 'sessions', 'BrowserPane Session Details'],
    ['/admin-new/recordings', 'recordings', 'BrowserPane Recordings'],
  ])('resolves %s', (pathname, activeId, title) => {
    expect(resolveAdminShellRoute(pathname)).toEqual({ activeId, title });
  });

  it('keeps the standalone session preview outside the admin shell', () => {
    expect(resolveAdminShellRoute('/admin-new/sessions/session-1/preview')).toBeNull();
  });
});
