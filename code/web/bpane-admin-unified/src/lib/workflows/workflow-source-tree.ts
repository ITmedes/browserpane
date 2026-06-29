import type { WorkflowDefinitionSourceFileResource } from './workflow-types';

export type WorkflowSourceTreeFileNode = {
  readonly kind: 'file';
  readonly name: string;
  readonly path: string;
  readonly file: WorkflowDefinitionSourceFileResource;
};

export type WorkflowSourceTreeDirectoryNode = {
  readonly kind: 'directory';
  readonly name: string;
  readonly path: string;
  readonly children: readonly WorkflowSourceTreeNode[];
};

export type WorkflowSourceTreeNode = WorkflowSourceTreeDirectoryNode | WorkflowSourceTreeFileNode;

export type WorkflowSourceTreeRow = {
  readonly node: WorkflowSourceTreeNode;
  readonly depth: number;
};

type MutableDirectoryNode = {
  readonly kind: 'directory';
  readonly name: string;
  readonly path: string;
  readonly directories: Map<string, MutableDirectoryNode>;
  readonly files: Map<string, WorkflowSourceTreeFileNode>;
};

export function buildWorkflowSourceTree(
  files: readonly WorkflowDefinitionSourceFileResource[],
): readonly WorkflowSourceTreeNode[] {
  const root = createDirectory('', '');
  for (const file of files) {
    insertFile(root, file);
  }
  return finalizeDirectory(root).children;
}

export function flattenWorkflowSourceTree(
  nodes: readonly WorkflowSourceTreeNode[],
  openDirectories: ReadonlySet<string>,
  depth = 0,
): readonly WorkflowSourceTreeRow[] {
  const rows: WorkflowSourceTreeRow[] = [];
  for (const node of nodes) {
    rows.push({ node, depth });
    if (node.kind === 'directory' && openDirectories.has(node.path)) {
      rows.push(...flattenWorkflowSourceTree(node.children, openDirectories, depth + 1));
    }
  }
  return rows;
}

export function sourcePathDirectoryAncestors(sourcePath: string | null): readonly string[] {
  if (!sourcePath) {
    return [];
  }
  const parts = splitPath(sourcePath);
  return parts.slice(0, -1).map((_, index) => parts.slice(0, index + 1).join('/'));
}

export function sourceTreeDirectoryPaths(nodes: readonly WorkflowSourceTreeNode[]): readonly string[] {
  const paths: string[] = [];
  for (const node of nodes) {
    if (node.kind !== 'directory') {
      continue;
    }
    paths.push(node.path);
    paths.push(...sourceTreeDirectoryPaths(node.children));
  }
  return paths;
}

function insertFile(root: MutableDirectoryNode, file: WorkflowDefinitionSourceFileResource): void {
  const parts = splitPath(file.path);
  if (parts.length === 0) {
    return;
  }
  let current = root;
  for (const part of parts.slice(0, -1)) {
    const path = current.path ? `${current.path}/${part}` : part;
    let next = current.directories.get(part);
    if (!next) {
      next = createDirectory(part, path);
      current.directories.set(part, next);
    }
    current = next;
  }
  const name = parts.at(-1);
  if (!name) {
    return;
  }
  current.files.set(name, {
    kind: 'file',
    name,
    path: file.path,
    file,
  });
}

function finalizeDirectory(directory: MutableDirectoryNode): WorkflowSourceTreeDirectoryNode {
  const directories = [...directory.directories.values()]
    .sort((left, right) => left.name.localeCompare(right.name))
    .map(finalizeDirectory);
  const files = [...directory.files.values()].sort((left, right) => left.name.localeCompare(right.name));
  return {
    kind: 'directory',
    name: directory.name,
    path: directory.path,
    children: [...directories, ...files],
  };
}

function createDirectory(name: string, path: string): MutableDirectoryNode {
  return {
    kind: 'directory',
    name,
    path,
    directories: new Map(),
    files: new Map(),
  };
}

function splitPath(path: string): readonly string[] {
  return path.split('/').filter(Boolean);
}
