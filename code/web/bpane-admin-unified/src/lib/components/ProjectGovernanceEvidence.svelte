<script lang="ts">
  import type {
    ProjectPolicyOptionsLoadState,
    ProjectRelatedSessionsLoadState,
    ProjectRelatedWorkflowRunsLoadState,
  } from '$lib/projects/project-detail-state';
  import type { ProjectResource } from '$lib/projects/project-types';
  import ProjectPolicyEvidence from './ProjectPolicyEvidence.svelte';
  import ProjectRelatedWorkEvidence from './ProjectRelatedWorkEvidence.svelte';
  import ProjectUsageEvidence from './ProjectUsageEvidence.svelte';

  type ProjectGovernanceEvidenceProps = {
    readonly project: ProjectResource;
    readonly policyOptionsState?: ProjectPolicyOptionsLoadState;
    readonly relatedSessionsState?: ProjectRelatedSessionsLoadState;
    readonly relatedWorkflowRunsState?: ProjectRelatedWorkflowRunsLoadState;
    readonly onRefreshRelatedWork?: () => void | Promise<void>;
  };

  let {
    project,
    policyOptionsState = { status: 'idle' },
    relatedSessionsState = { status: 'idle' },
    relatedWorkflowRunsState = { status: 'idle' },
    onRefreshRelatedWork,
  }: ProjectGovernanceEvidenceProps = $props();
</script>

<section
  class="overflow-hidden rounded-md border border-admin-border bg-admin-panel"
  data-testid="project-governance-evidence"
>
  <header class="border-b border-admin-border px-4 py-4 sm:px-5">
    <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Observed state</p>
    <h3 class="m-0 mt-1 text-base font-semibold text-admin-ink">Operational governance</h3>
    <p class="m-0 mt-1 max-w-3xl text-sm leading-6 text-admin-muted">
      Current pressure, effective project policy, and related work. Gateway admission remains authoritative.
    </p>
  </header>

  <ProjectUsageEvidence {project} />
  <ProjectPolicyEvidence {project} {policyOptionsState} />
  <ProjectRelatedWorkEvidence
    projectId={project.id}
    sessionsState={relatedSessionsState}
    workflowRunsState={relatedWorkflowRunsState}
    onRefresh={onRefreshRelatedWork}
  />
</section>
