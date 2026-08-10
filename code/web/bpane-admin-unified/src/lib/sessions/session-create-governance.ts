import type {
  ProjectResourceDecision,
  ProjectResourceKind,
  ProjectUsageGovernanceModel,
} from '$lib/projects/project-governance-types';
import { ProjectPolicyEvaluator } from '$lib/projects/project-policy-evaluator';
import { ProjectUsagePresenter } from '$lib/projects/project-usage-presenter';
import type { ProjectPolicyOption, ProjectResource } from '$lib/projects/project-types';

import type { SessionCreateOptions } from './session-create-view-model';

export type SessionCreateResourceChoice = {
  readonly id: string;
  readonly name: string;
  readonly state: string | null;
  readonly scope: string | null;
  readonly disabled: boolean;
  readonly reason: string;
  readonly decision: ProjectResourceDecision;
};

export type SessionCreateGovernanceModel = {
  readonly project: ProjectResource | null;
  readonly projectUsage: ProjectUsageGovernanceModel | null;
  readonly sessionTemplates: readonly SessionCreateResourceChoice[];
  readonly browserContexts: readonly SessionCreateResourceChoice[];
  readonly egressProfiles: readonly SessionCreateResourceChoice[];
};

export class SessionCreateGovernancePresenter {
  private readonly policyEvaluator: ProjectPolicyEvaluator;
  private readonly usagePresenter: ProjectUsagePresenter;

  public constructor(
    policyEvaluator = new ProjectPolicyEvaluator(),
    usagePresenter = new ProjectUsagePresenter(),
  ) {
    this.policyEvaluator = policyEvaluator;
    this.usagePresenter = usagePresenter;
  }

  public build(
    projectId: string,
    options: SessionCreateOptions,
  ): SessionCreateGovernanceModel {
    const project = options.projects.find((candidate) => candidate.id === projectId) ?? null;
    return {
      project,
      projectUsage: project ? this.usagePresenter.build(project) : null,
      sessionTemplates: this.choices(project, 'session_template', options.sessionTemplates),
      browserContexts: this.choices(project, 'browser_context', options.browserContexts),
      egressProfiles: this.choices(project, 'egress_profile', options.egressProfiles),
    };
  }

  public evaluateResource(
    project: ProjectResource | null,
    kind: ProjectResourceKind,
    option: ProjectPolicyOption,
  ): ProjectResourceDecision {
    return project
      ? this.policyEvaluator.evaluateOption(project, kind, option)
      : this.policyEvaluator.evaluateOwnerScopeOption(kind, option);
  }

  private choices(
    project: ProjectResource | null,
    kind: ProjectResourceKind,
    options: readonly ProjectPolicyOption[],
  ): readonly SessionCreateResourceChoice[] {
    return options.map((option) => {
      const decision = this.evaluateResource(project, kind, option);
      return {
        id: option.id,
        name: option.name,
        state: option.state,
        scope: option.scope,
        disabled: !decision.allowed,
        reason: decision.reason,
        decision,
      };
    });
  }
}
