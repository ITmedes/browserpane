import type { ProjectPolicy, ProjectResource } from '$lib/projects/project-types';
import type { ProjectTone } from '$lib/projects/project-formatters';
import { ProjectPolicyEvaluator } from '$lib/projects/project-policy-evaluator';
import type { SessionResource, SessionStatus } from './session-types';

export type SessionPolicyFact = {
  readonly label: string;
  readonly value: string;
  readonly description: string;
  readonly tone: ProjectTone;
  readonly testId: string;
};

export type SessionPolicySection = {
  readonly title: string;
  readonly description: string;
  readonly testId: string;
  readonly facts: readonly SessionPolicyFact[];
};

export type SessionPolicyModel = {
  readonly scopeLabel: string;
  readonly scopeTone: ProjectTone;
  readonly projectHref: string | null;
  readonly sections: readonly SessionPolicySection[];
};

const projectPolicyEvaluator = new ProjectPolicyEvaluator();

const CAPABILITY_LABELS: Readonly<Record<keyof SessionResource['capabilities'], string>> = {
  browser_input: 'Browser input',
  clipboard: 'Clipboard',
  audio: 'Desktop audio',
  microphone: 'Microphone ingress',
  camera: 'Camera ingress',
  file_transfer: 'Browser file transfer',
  resize: 'Display resize',
};

export function buildSessionPolicyModel(
  session: SessionResource,
  status: SessionStatus | null,
  project: ProjectResource | null,
): SessionPolicyModel {
  return {
    scopeLabel: project
      ? `${project.name} project policy`
      : session.project_id
        ? `${session.project?.name ?? 'Project'} policy unavailable`
        : 'Owner-scoped defaults',
    scopeTone: project ? projectStateTone(project.state) : session.project_id ? 'warning' : 'neutral',
    projectHref: session.project_id
      ? `/admin-new/projects/${encodeURIComponent(session.project_id)}`
      : null,
    sections: [
      capabilitySection(session, project),
      operationSection(project, Boolean(session.project_id)),
      resourceSection(session, project),
      admissionSection(session, status, project),
      browserPolicySection(session),
    ],
  };
}

function capabilitySection(session: SessionResource, project: ProjectResource | null): SessionPolicySection {
  return {
    title: 'Effective capabilities',
    description: 'Capabilities reported by the session resource and enforced for new client connections.',
    testId: 'session-policy-capabilities',
    facts: Object.entries(session.capabilities).map(([key, enabled]) => {
      const capability = key as keyof SessionResource['capabilities'];
      return fact(
        CAPABILITY_LABELS[capability],
        enabled ? 'Enabled' : 'Disabled',
        capabilityDescription(capability, enabled, project?.policy ?? null),
        enabled ? 'success' : 'warning',
        `session-policy-capability-${capability}`,
      );
    }),
  };
}

function operationSection(project: ProjectResource | null, projectBound: boolean): SessionPolicySection {
  const operations = project ? projectPolicyEvaluator.operationPolicies(project) : [];
  return {
    title: 'Project operations',
    description: project
      ? 'Project policy gates applied to browser transfer, session files, and manual recording operations.'
      : projectBound
        ? 'Project operation policy evidence could not be loaded for this project-bound session.'
        : 'This owner-scoped session has no project operation policy attached.',
    testId: 'session-policy-operations',
    facts: project
      ? operations.map((operation) => fact(
          operation.label,
          operation.allowed ? 'Allowed' : 'Blocked',
          operation.reason,
          operation.tone,
          `session-policy-${operationTestId(operation.id)}`,
        ))
      : [
          unavailableOperationFact('Browser uploads', 'browser-upload', projectBound),
          unavailableOperationFact('Browser downloads', 'browser-download', projectBound),
          unavailableOperationFact('Session file bindings', 'session-file-bindings', projectBound),
          unavailableOperationFact('Manual recording starts', 'manual-recordings', projectBound),
        ],
  };
}

function resourceSection(session: SessionResource, project: ProjectResource | null): SessionPolicySection {
  const policy = project?.policy ?? null;
  return {
    title: 'Resource policy',
    description: 'Selected session resources compared with project allowlists. Empty allowlists are unrestricted.',
    testId: 'session-policy-resources',
    facts: [
      selectedResourceFact(
        'Session template',
        session.template_id ?? null,
        policy?.allowed_session_template_ids ?? [],
        'template',
      ),
      selectedResourceFact(
        'Browser context',
        session.browser_context.context_id ?? null,
        policy?.allowed_browser_context_ids ?? [],
        'browser-context',
      ),
      selectedResourceFact(
        'Egress profile',
        session.effective_egress?.profile_id ?? session.network_identity?.egress_profile_id ?? null,
        policy?.allowed_egress_profile_ids ?? [],
        'egress-profile',
      ),
      allowlistFact('Extensions', policy?.allowed_extension_ids ?? [], 'extensions'),
      allowlistFact('File workspaces', policy?.allowed_file_workspace_ids ?? [], 'file-workspaces'),
    ],
  };
}

function admissionSection(
  session: SessionResource,
  status: SessionStatus | null,
  project: ProjectResource | null,
): SessionPolicySection {
  const admission = status?.admission ?? session.admission ?? null;
  const alerts = project?.usage.alerts ?? [];
  return {
    title: 'Admission and budgets',
    description: 'The latest admission decision and the current project quota enforcement evidence.',
    testId: 'session-policy-admission',
    facts: [
      fact(
        'Admission',
        admission ? `${admission.state}: ${admission.reason_code}` : 'Owner scope',
        admission?.message || 'No project admission decision applies to this session.',
        admissionTone(admission?.state ?? null),
        'session-policy-admission-decision',
      ),
      fact(
        'Usage budget enforcement',
        project?.policy.usage_budget_enforcement ?? 'Not applicable',
        project
          ? 'Controls whether exhausted project budgets warn or block new session creation.'
          : 'Owner-scoped sessions are not governed by a project usage budget.',
        project?.policy.usage_budget_enforcement === 'block_session_creation' ? 'warning' : 'neutral',
        'session-policy-budget-enforcement',
      ),
      fact(
        'Usage alerts',
        alerts.length === 0 ? 'None' : `${alerts.length} active`,
        alerts.length === 0
          ? 'No project usage threshold is currently reported as approaching or exceeded.'
          : alerts.map((alert) => alert.message).join(' '),
        alerts.length === 0 ? 'success' : 'warning',
        'session-policy-usage-alerts',
      ),
      fact(
        'Stop eligibility',
        status?.stop_eligibility.allowed ? 'Allowed' : status ? 'Blocked' : 'Not loaded',
        stopEligibilityDescription(status),
        status?.stop_eligibility.allowed ? 'success' : status ? 'warning' : 'neutral',
        'session-policy-stop-eligibility',
      ),
    ],
  };
}

function browserPolicySection(session: SessionResource): SessionPolicySection {
  const dockerBacked = isDockerBacked(session);
  const evidenceDescription = dockerBacked
    ? 'Docker-backed host startup validates deny policies for local file URLs and the File System Access API.'
    : 'This runtime shape does not provide the docker-backed managed Chromium policy guarantee.';
  return {
    title: 'Managed browser policy',
    description: 'Runtime launch evidence for local filesystem isolation. This is not an active browser probe.',
    testId: 'session-policy-browser-runtime',
    facts: [
      fact(
        'Local file access mode',
        dockerBacked ? 'Startup-enforced deny-all' : 'Not guaranteed',
        evidenceDescription,
        dockerBacked ? 'success' : 'warning',
        'session-policy-local-file-mode',
      ),
      fact(
        'file:// navigation',
        dockerBacked ? 'Blocked by policy' : 'Unknown',
        evidenceDescription,
        dockerBacked ? 'success' : 'neutral',
        'session-policy-file-url',
      ),
      fact(
        'File System read/write',
        dockerBacked ? 'Blocked by policy' : 'Unknown',
        evidenceDescription,
        dockerBacked ? 'success' : 'neutral',
        'session-policy-file-system-access',
      ),
      fact(
        'Runtime evidence',
        `${session.runtime.binding} / ${session.runtime.compatibility_mode}`,
        session.runtime.cdp_endpoint
          ? 'A runtime CDP endpoint is assigned; active probes remain an explicit operator action.'
          : 'No runtime CDP endpoint is currently assigned.',
        dockerBacked ? 'success' : 'neutral',
        'session-policy-runtime-evidence',
      ),
    ],
  };
}

function capabilityDescription(
  capability: keyof SessionResource['capabilities'],
  enabled: boolean,
  policy: ProjectPolicy | null,
): string {
  if (capability !== 'file_transfer' || enabled || !policy) {
    return enabled
      ? 'The effective session capability permits this operation for authorized interactive clients.'
      : 'The effective session capability denies this operation for client connections.';
  }
  const blockers = [
    ...(!policy.allow_browser_uploads ? ['browser uploads'] : []),
    ...(!policy.allow_browser_downloads ? ['browser downloads'] : []),
  ];
  return blockers.length > 0
    ? `Disabled because the project blocks ${blockers.join(' and ')}.`
    : 'Disabled by session configuration even though project transfer operations are allowed.';
}

function unavailableOperationFact(label: string, id: string, projectBound: boolean): SessionPolicyFact {
  return fact(
    label,
    projectBound ? 'Unavailable' : 'Owner scope',
    projectBound
      ? 'The project restriction could not be confirmed; the gateway remains authoritative.'
      : 'No project restriction applies; the effective session capability remains authoritative.',
    projectBound ? 'warning' : 'neutral',
    `session-policy-${id}`,
  );
}

function operationTestId(id: string): string {
  switch (id) {
    case 'browser_uploads': return 'browser-upload';
    case 'browser_downloads': return 'browser-download';
    case 'session_file_bindings': return 'session-file-bindings';
    case 'manual_recordings': return 'manual-recordings';
    default: return id;
  }
}

function selectedResourceFact(
  label: string,
  selectedId: string | null,
  allowedIds: readonly string[],
  id: string,
): SessionPolicyFact {
  if (!selectedId) {
    return fact(label, 'None', 'No resource is selected for this session.', 'neutral', `session-policy-${id}`);
  }
  if (allowedIds.length === 0) {
    return fact(
      label,
      selectedId,
      'The project allowlist is unrestricted for this resource type.',
      'success',
      `session-policy-${id}`,
    );
  }
  const allowed = allowedIds.includes(selectedId);
  return fact(
    label,
    selectedId,
    allowed
      ? 'The selected resource is included in the project allowlist.'
      : 'The selected resource is outside the current project allowlist; inspect legacy or changed policy state.',
    allowed ? 'success' : 'danger',
    `session-policy-${id}`,
  );
}

function allowlistFact(label: string, allowedIds: readonly string[], id: string): SessionPolicyFact {
  return fact(
    label,
    allowedIds.length === 0 ? 'Unrestricted' : `${allowedIds.length} allowed`,
    allowedIds.length === 0
      ? 'The project does not restrict this resource type.'
      : `Allowed ids: ${allowedIds.join(', ')}`,
    allowedIds.length === 0 ? 'neutral' : 'success',
    `session-policy-${id}`,
  );
}

function stopEligibilityDescription(status: SessionStatus | null): string {
  if (!status) {
    return 'Live session status could not be loaded.';
  }
  if (status.stop_eligibility.allowed) {
    return 'No live connection or runtime blocker currently prevents a graceful stop.';
  }
  if (status.stop_eligibility.blockers.length === 0) {
    return 'The gateway reports a stop blocker without additional details.';
  }
  return status.stop_eligibility.blockers
    .map((blocker) => `${blocker.count} ${blocker.kind}`)
    .join(', ');
}

function isDockerBacked(session: SessionResource): boolean {
  return session.runtime.binding.includes('docker')
    || session.runtime.compatibility_mode.includes('docker')
    || session.connect.compatibility_mode.includes('pool');
}

function projectStateTone(state: string): ProjectTone {
  return state === 'active' ? 'success' : 'warning';
}

function admissionTone(state: string | null): ProjectTone {
  if (state === 'allowed') {
    return 'success';
  }
  if (state === 'rejected' || state === 'denied') {
    return 'danger';
  }
  return state ? 'warning' : 'neutral';
}

function fact(
  label: string,
  value: string,
  description: string,
  tone: ProjectTone,
  testId: string,
): SessionPolicyFact {
  return { label, value, description, tone, testId };
}
