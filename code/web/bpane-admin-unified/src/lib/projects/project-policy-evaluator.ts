import type {
  ProjectAllowlistSummary,
  ProjectOperationPolicy,
  ProjectResourceDecision,
  ProjectResourceKind,
} from './project-governance-types';
import type { ProjectPolicyOption, ProjectResource } from './project-types';

const BLOCKED_RESOURCE_STATES = new Set(['archived', 'deleted', 'disabled']);

const RESOURCE_LABELS: Readonly<Record<ProjectResourceKind, string>> = {
  session_template: 'session templates',
  browser_context: 'browser contexts',
  egress_profile: 'egress profiles',
  extension: 'extensions',
  file_workspace: 'file workspaces',
};

export class ProjectPolicyEvaluator {
  public evaluateOption(
    project: ProjectResource,
    kind: ProjectResourceKind,
    option: ProjectPolicyOption,
  ): ProjectResourceDecision {
    const state = option.state?.toLowerCase() ?? null;
    if (state && BLOCKED_RESOURCE_STATES.has(state)) {
      return this.decision(option, kind, false, 'blocked_resource_state',
        `${option.name} is ${state} and cannot be selected.`, 'warning');
    }
    if (option.projectId && option.projectId !== project.id) {
      return this.decision(option, kind, false, 'blocked_project_scope',
        `${option.name} belongs to another project.`, 'danger');
    }
    const allowedIds = this.allowedIds(project, kind);
    if (allowedIds.length === 0) {
      return this.decision(option, kind, true, 'allowed_unrestricted',
        `${project.name} does not restrict ${RESOURCE_LABELS[kind]}.`, 'neutral');
    }
    if (allowedIds.includes(option.id)) {
      return this.decision(option, kind, true, 'allowed_by_policy',
        `${option.name} is included in the ${project.name} allowlist.`, 'success');
    }
    return this.decision(option, kind, false, 'blocked_by_policy',
      `${option.name} is not included in the ${project.name} allowlist.`, 'danger');
  }

  public evaluateReference(
    project: ProjectResource,
    kind: ProjectResourceKind,
    resourceId: string,
    options: readonly ProjectPolicyOption[],
  ): ProjectResourceDecision {
    const option = options.find((candidate) => candidate.id === resourceId);
    if (option) {
      return this.evaluateOption(project, kind, option);
    }
    return {
      id: resourceId,
      kind,
      name: resourceId,
      allowed: false,
      code: 'missing_reference',
      reason: `Configured ${this.singularLabel(kind)} ${resourceId} is not visible to the current operator.`,
      tone: 'warning',
    };
  }

  public summarizeAllowlist(
    project: ProjectResource,
    kind: ProjectResourceKind,
    options: readonly ProjectPolicyOption[],
  ): ProjectAllowlistSummary {
    const allowedIds = this.allowedIds(project, kind);
    return {
      kind,
      label: RESOURCE_LABELS[kind],
      mode: allowedIds.length === 0 ? 'unrestricted' : 'restricted',
      configuredCount: allowedIds.length,
      resources: allowedIds.map((id) => this.evaluateReference(project, kind, id, options)),
    };
  }

  public operationPolicies(project: ProjectResource): readonly ProjectOperationPolicy[] {
    return [
      this.operation('browser_uploads', 'Browser uploads', project.policy.allow_browser_uploads),
      this.operation('browser_downloads', 'Browser downloads', project.policy.allow_browser_downloads),
      this.operation('session_file_bindings', 'Session file bindings', project.policy.allow_session_file_bindings),
      this.operation('manual_recordings', 'Manual recording starts', project.policy.allow_manual_recordings),
    ];
  }

  private allowedIds(project: ProjectResource, kind: ProjectResourceKind): readonly string[] {
    switch (kind) {
      case 'session_template': return project.policy.allowed_session_template_ids;
      case 'browser_context': return project.policy.allowed_browser_context_ids;
      case 'egress_profile': return project.policy.allowed_egress_profile_ids;
      case 'extension': return project.policy.allowed_extension_ids;
      case 'file_workspace': return project.policy.allowed_file_workspace_ids;
    }
  }

  private singularLabel(kind: ProjectResourceKind): string {
    return RESOURCE_LABELS[kind].replace(/s$/u, '');
  }

  private operation(
    id: ProjectOperationPolicy['id'],
    label: string,
    allowed: boolean,
  ): ProjectOperationPolicy {
    return {
      id,
      label,
      allowed,
      reason: allowed
        ? `The project permits ${label.toLowerCase()} when the selected resource and session capability also allow them.`
        : `The project blocks ${label.toLowerCase()} for its sessions.`,
      tone: allowed ? 'success' : 'warning',
    };
  }

  private decision(
    option: ProjectPolicyOption,
    kind: ProjectResourceKind,
    allowed: boolean,
    code: ProjectResourceDecision['code'],
    reason: string,
    tone: ProjectResourceDecision['tone'],
  ): ProjectResourceDecision {
    return { id: option.id, kind, name: option.name, allowed, code, reason, tone };
  }
}
