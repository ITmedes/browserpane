import type { ProjectPolicy, ProjectResource } from '$lib/projects/project-types';
import type { ProjectTone } from '$lib/projects/project-formatters';
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
  readonly sections: readonly SessionPolicySection[];
};

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
    scopeLabel: project ? `${project.name} project policy` : 'Owner-scoped defaults',
    scopeTone: project ? projectStateTone(project.state) : 'neutral',
    sections: [
      capabilitySection(session, project),
      operationSection(project),
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

function operationSection(project: ProjectResource | null): SessionPolicySection {
  const policy = project?.policy ?? null;
  return {
    title: 'Project operations',
    description: project
      ? 'Project policy gates applied to browser transfer, session files, and manual recording operations.'
      : 'This owner-scoped session has no project operation policy attached.',
    testId: 'session-policy-operations',
    facts: [
      operationFact('Browser uploads', policy?.allow_browser_uploads, 'browser-upload'),
      operationFact('Browser downloads', policy?.allow_browser_downloads, 'browser-download'),
      operationFact('Session file bindings', policy?.allow_session_file_bindings, 'session-file-bindings'),
      operationFact('Manual recording starts', policy?.allow_manual_recordings, 'manual-recordings'),
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

function operationFact(label: string, allowed: boolean | undefined, id: string): SessionPolicyFact {
  if (allowed === undefined) {
    return fact(
      label,
      'Owner scope',
      'No project restriction applies; the effective session capability remains authoritative.',
      'neutral',
      `session-policy-${id}`,
    );
  }
  return fact(
    label,
    allowed ? 'Allowed' : 'Blocked',
    allowed
      ? 'The project policy permits this operation when the session capability also allows it.'
      : 'The project policy denies this operation for sessions in this project.',
    allowed ? 'success' : 'warning',
    `session-policy-${id}`,
  );
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
  if (state === 'denied') {
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
