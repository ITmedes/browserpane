export type AdminShellRoute = {
  readonly activeId: string;
  readonly title: string;
};

export function resolveAdminShellRoute(pathname: string): AdminShellRoute | null {
  const route = normalizeRoute(pathname);
  if (/^\/sessions\/[^/]+\/preview$/.test(route)) {
    return null;
  }
  if (route === '/') {
    return { activeId: 'dashboard', title: 'BrowserPane Dashboard' };
  }
  if (route === '/projects') {
    return { activeId: 'projects', title: 'BrowserPane Projects' };
  }
  if (route === '/projects/new') {
    return { activeId: 'projects', title: 'BrowserPane New Project' };
  }
  if (route.startsWith('/projects/')) {
    return { activeId: 'projects', title: 'BrowserPane Project Details' };
  }
  if (route === '/browser-contexts') {
    return { activeId: 'contexts', title: 'BrowserPane Browser Contexts' };
  }
  if (route === '/browser-contexts/new') {
    return { activeId: 'contexts', title: 'BrowserPane New Browser Context' };
  }
  if (route.startsWith('/browser-contexts/')) {
    return { activeId: 'contexts', title: 'BrowserPane Browser Context Details' };
  }
  if (route === '/egress') {
    return { activeId: 'egress', title: 'BrowserPane Egress Profiles' };
  }
  if (route === '/egress/new') {
    return { activeId: 'egress', title: 'BrowserPane New Egress Profile' };
  }
  if (route.startsWith('/egress/')) {
    return { activeId: 'egress', title: 'BrowserPane Egress Profile Details' };
  }
  if (route === '/files/workspaces') {
    return { activeId: 'workspaces', title: 'BrowserPane File Workspaces' };
  }
  if (route === '/files/workspaces/new') {
    return { activeId: 'workspaces', title: 'BrowserPane New File Workspace' };
  }
  if (route.startsWith('/files/workspaces/')) {
    return { activeId: 'workspaces', title: 'BrowserPane File Workspace Details' };
  }
  if (route === '/workflows') {
    return { activeId: 'workflows', title: 'BrowserPane Workflows' };
  }
  if (route.startsWith('/workflows/')) {
    return { activeId: 'workflows', title: 'BrowserPane Workflow Details' };
  }
  if (route === '/runs' || route.startsWith('/runs/')) {
    return { activeId: 'runs', title: route === '/runs'
      ? 'BrowserPane Workflow Runs'
      : 'BrowserPane Workflow Run Details' };
  }
  if (route === '/workflow-runs' || route.startsWith('/workflow-runs/')) {
    return { activeId: 'runs', title: route === '/workflow-runs'
      ? 'BrowserPane Workflow Runs'
      : 'BrowserPane Workflow Run Details' };
  }
  if (route === '/sessions') {
    return { activeId: 'sessions', title: 'BrowserPane Sessions' };
  }
  if (route === '/sessions/new') {
    return { activeId: 'sessions', title: 'BrowserPane New Session' };
  }
  if (/^\/sessions\/[^/]+\/live$/.test(route)) {
    return { activeId: 'sessions', title: 'BrowserPane Live Session' };
  }
  if (/^\/sessions\/[^/]+\/files$/.test(route)) {
    return { activeId: 'sessions', title: 'BrowserPane Session Files' };
  }
  if (route.startsWith('/sessions/')) {
    return { activeId: 'sessions', title: 'BrowserPane Session Details' };
  }
  if (route === '/recordings') {
    return { activeId: 'recordings', title: 'BrowserPane Recordings' };
  }
  return { activeId: 'dashboard', title: 'BrowserPane Admin' };
}

function normalizeRoute(pathname: string): string {
  const withoutBase = pathname.replace(/^\/admin-new(?=\/|$)/, '');
  const normalized = `/${withoutBase}`.replace(/\/{2,}/g, '/').replace(/\/$/, '');
  return normalized || '/';
}
