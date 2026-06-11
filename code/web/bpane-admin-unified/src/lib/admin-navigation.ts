export type NavIconKey =
  | 'activity'
  | 'book-open'
  | 'boxes'
  | 'check'
  | 'cpu'
  | 'folder'
  | 'gauge'
  | 'globe'
  | 'info'
  | 'key'
  | 'layers'
  | 'list'
  | 'monitor'
  | 'network'
  | 'shield'
  | 'terminal'
  | 'workflow';

export type NavGroupName = 'Operate' | 'Resources' | 'Govern' | 'Docs';

export type NavItem = {
  id: string;
  label: string;
  route: string;
  icon: NavIconKey;
  active?: boolean;
};

export type NavGroup = {
  group: NavGroupName | null;
  items: NavItem[];
};

export const navGroups: NavGroup[] = [
  {
    group: null,
    items: [
      { id: 'dashboard', label: 'Dashboard', route: '/admin-new/', icon: 'activity', active: true },
    ],
  },
  {
    group: 'Operate',
    items: [
      { id: 'sessions', label: 'Sessions', route: '/admin-new/sessions', icon: 'cpu' },
      { id: 'workflows', label: 'Workflows', route: '/admin-new/workflows', icon: 'workflow' },
      { id: 'runs', label: 'Workflow runs', route: '/admin-new/runs', icon: 'list' },
    ],
  },
  {
    group: 'Resources',
    items: [
      { id: 'projects', label: 'Projects', route: '/admin-new/projects', icon: 'layers' },
      { id: 'contexts', label: 'Browser contexts', route: '/admin-new/contexts', icon: 'globe' },
      { id: 'egress', label: 'Egress profiles', route: '/admin-new/egress', icon: 'network' },
      { id: 'workspaces', label: 'File workspaces', route: '/admin-new/workspaces', icon: 'folder' },
    ],
  },
  {
    group: 'Govern',
    items: [
      { id: 'identity', label: 'Identity & access', route: '/admin-new/identity', icon: 'shield' },
      { id: 'api', label: 'API reference', route: '/admin-new/api', icon: 'key' },
    ],
  },
  {
    group: 'Docs',
    items: [
      { id: 'memo', label: 'Design memo', route: '/admin-new/docs', icon: 'info' },
      { id: 'coverage', label: 'API coverage', route: '/admin-new/coverage', icon: 'check' },
    ],
  },
];

export const allNavItems = navGroups.flatMap((group) => group.items);

export const primaryNav: NavItem[] = allNavItems.filter((item) =>
  ['dashboard', 'sessions', 'workflows', 'runs', 'projects', 'contexts', 'egress', 'workspaces'].includes(item.id),
);

export const secondaryNav: NavItem[] = allNavItems.filter((item) =>
  ['identity', 'api', 'memo', 'coverage'].includes(item.id),
);
