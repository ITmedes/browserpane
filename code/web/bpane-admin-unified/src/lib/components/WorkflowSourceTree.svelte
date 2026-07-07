<script lang="ts">
  import { ChevronRight, FileText, Folder, FolderOpen } from '@lucide/svelte';
  import {
    buildWorkflowSourceTree,
    flattenWorkflowSourceTree,
    sourcePathDirectoryAncestors,
  } from '$lib/workflows/workflow-source-tree';
  import type {
    WorkflowDefinitionSourceFileResource,
  } from '$lib/workflows/workflow-types';

  type WorkflowSourceTreeProps = {
    readonly files: readonly WorkflowDefinitionSourceFileResource[];
    readonly selectedPath?: string | null;
    readonly onSelectFile?: (path: string) => void | Promise<void>;
    readonly formatFileMeta?: (file: WorkflowDefinitionSourceFileResource) => string;
  };

  let {
    files,
    selectedPath = null,
    onSelectFile,
    formatFileMeta = defaultFileMeta,
  }: WorkflowSourceTreeProps = $props();

  let openDirectories = $state<ReadonlySet<string>>(new Set());

  const tree = $derived(buildWorkflowSourceTree(files));
  const effectiveOpenDirectories = $derived(mergeOpenDirectories(
    openDirectories,
    sourcePathDirectoryAncestors(selectedPath),
  ));
  const rows = $derived(flattenWorkflowSourceTree(tree, effectiveOpenDirectories));

  function toggleDirectory(path: string): void {
    const next = new Set(openDirectories);
    if (next.has(path)) {
      next.delete(path);
    } else {
      next.add(path);
    }
    openDirectories = next;
  }

  function selectFile(path: string): void {
    void onSelectFile?.(path);
  }

  function defaultFileMeta(file: WorkflowDefinitionSourceFileResource): string {
    return `${file.language} · ${file.byte_count} B`;
  }

  function mergeOpenDirectories(
    userOpenDirectories: ReadonlySet<string>,
    selectedAncestors: readonly string[],
  ): ReadonlySet<string> {
    return new Set([...userOpenDirectories, ...selectedAncestors]);
  }

  function rowInset(depth: number): string {
    return `padding-left: ${Math.max(depth, 0) * 0.85}rem`;
  }
</script>

<div
  class="mt-3 grid max-h-[520px] gap-1 overflow-auto pr-1"
  role="tree"
  aria-label="Workflow source files"
  data-testid="workflow-code-file-list"
>
  {#each rows as row (`${row.node.kind}:${row.node.path}`)}
    {#if row.node.kind === 'directory'}
      {@const isOpen = effectiveOpenDirectories.has(row.node.path)}
      <button
        class="group flex min-w-0 items-center gap-2 rounded-md border border-transparent bg-transparent px-2 py-2 text-left text-xs text-admin-muted transition hover:border-admin-border hover:bg-admin-soft"
        type="button"
        role="treeitem"
        aria-level={row.depth + 1}
        aria-expanded={isOpen}
        aria-selected="false"
        data-testid="workflow-code-folder-row"
        data-source-path={row.node.path}
        onclick={() => toggleDirectory(row.node.path)}
      >
        <span class="flex min-w-0 flex-1 items-center gap-2" style={rowInset(row.depth)}>
          <ChevronRight
            class={`shrink-0 text-admin-muted transition-transform ${isOpen ? 'rotate-90' : ''}`}
            size={14}
            strokeWidth={1.9}
          />
          {#if isOpen}
            <FolderOpen class="shrink-0 text-admin-accent" size={15} strokeWidth={1.8} />
          {:else}
            <Folder class="shrink-0 text-admin-muted" size={15} strokeWidth={1.8} />
          {/if}
          <span class="min-w-0 truncate font-medium text-admin-ink">{row.node.name}</span>
        </span>
      </button>
    {:else}
      {@const selected = selectedPath === row.node.path}
      <button
        class={`group grid min-w-0 gap-1 rounded-md border px-2 py-2 text-left text-xs transition ${
          selected
            ? 'border-admin-accent bg-admin-panel text-admin-ink shadow-sm'
            : 'border-transparent bg-transparent text-admin-muted hover:border-admin-border hover:bg-admin-soft'
        }`}
        type="button"
        role="treeitem"
        aria-level={row.depth + 1}
        aria-selected={selected}
        data-testid="workflow-code-file-row"
        data-source-path={row.node.path}
        data-selected={selected ? 'true' : 'false'}
        onclick={() => selectFile(row.node.path)}
      >
        <span class="flex min-w-0 items-start gap-2" style={rowInset(row.depth)}>
          <FileText
            class={`mt-0.5 shrink-0 ${selected ? 'text-admin-accent' : 'text-admin-muted'}`}
            size={14}
            strokeWidth={1.8}
          />
          <span class="grid min-w-0 flex-1 gap-0.5">
            <span class="flex min-w-0 items-start justify-between gap-2">
              <span class="min-w-0 truncate font-mono text-[11px] leading-4">{row.node.name}</span>
              {#if row.node.file.entrypoint}
                <span class="shrink-0 rounded-full bg-emerald-50 px-1.5 py-0.5 text-[10px] font-semibold text-emerald-700 ring-1 ring-emerald-200">
                  entry
                </span>
              {/if}
            </span>
            <span class="truncate text-[11px] text-admin-muted">{formatFileMeta(row.node.file)}</span>
          </span>
        </span>
      </button>
    {/if}
  {/each}
</div>
