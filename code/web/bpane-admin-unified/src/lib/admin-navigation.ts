export type NavIconKey =
  | 'activity'
  | 'book-open'
  | 'boxes'
  | 'folder'
  | 'gauge'
  | 'layers'
  | 'monitor'
  | 'network'
  | 'shield'
  | 'terminal';

export type NavItem = {
  id: string;
  label: string;
  route: string;
  icon: NavIconKey;
  active?: boolean;
};

export const primaryNav: NavItem[] = [
  { id: 'dashboard', label: 'Dashboard', route: '/admin-new/', icon: 'gauge', active: true },
  { id: 'projects', label: 'Projects', route: '/admin-new/projects', icon: 'layers' },
  { id: 'workspaces', label: 'File workspaces', route: '/admin-new/workspaces', icon: 'folder' },
  { id: 'contexts', label: 'Browser contexts', route: '/admin-new/contexts', icon: 'boxes' },
  { id: 'egress', label: 'Egress profiles', route: '/admin-new/egress', icon: 'network' },
  { id: 'sessions', label: 'Sessions', route: '/admin-new/sessions', icon: 'monitor' },
  { id: 'runs', label: 'Workflow runs', route: '/admin-new/runs', icon: 'activity' },

];

export const secondaryNav: NavItem[] = [
  { id: 'identity', label: 'Identity', route: '/admin-new/identity', icon: 'shield' },
  { id: 'api', label: 'API reference', route: '/admin-new/api', icon: 'terminal' },
  { id: 'docs', label: 'Design memo', route: '/admin-new/docs', icon: 'book-open' },
];
