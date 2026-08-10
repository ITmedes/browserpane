import type { ProjectPolicyOption, ProjectResource } from '$lib/projects/project-types';
import type { ProjectResourceKind } from '$lib/projects/project-governance-types';
import { SessionCreateGovernancePresenter } from './session-create-governance';
import type { CreateSessionRequest, SessionCapabilities, SessionRecordingPolicy } from './session-types';

export type SessionCreateDraft = {
  projectId: string;
  templateId: string;
  ownerMode: '' | 'collaborative' | 'exclusive_browser_owner';
  browserContextMode: '' | 'fresh' | 'ephemeral' | 'reusable';
  browserContextId: string;
  egressProfileId: string;
  idleTimeoutSec: string;
  viewportWidth: string;
  viewportHeight: string;
  capabilityBrowserInput: boolean;
  capabilityClipboard: boolean;
  capabilityAudio: boolean;
  capabilityMicrophone: boolean;
  capabilityCamera: boolean;
  capabilityFileTransfer: boolean;
  capabilityResize: boolean;
  recordingEnabled: boolean;
  recordingRetentionSec: string;
  locale: string;
  languagesText: string;
  timezone: string;
  browserIdentity: string;
  userAgent: string;
  labelsText: string;
};

const DEFAULT_CAPABILITIES: SessionCapabilities = {
  browser_input: true,
  clipboard: true,
  audio: true,
  microphone: true,
  camera: true,
  file_transfer: true,
  resize: true,
};

export type SessionCreateOptions = {
  readonly projects: readonly ProjectResource[];
  readonly sessionTemplates: readonly ProjectPolicyOption[];
  readonly browserContexts: readonly ProjectPolicyOption[];
  readonly egressProfiles: readonly ProjectPolicyOption[];
};

export type SessionCreateOptionsLoadState =
  | { readonly status: 'idle' }
  | { readonly status: 'loading' }
  | { readonly status: 'ready'; readonly options: SessionCreateOptions }
  | { readonly status: 'error'; readonly message: string };

export type SessionCreateValidationField =
  | 'projectId'
  | 'templateId'
  | 'browserContextId'
  | 'egressProfileId'
  | 'idleTimeoutSec'
  | 'recordingRetentionSec'
  | 'viewport'
  | 'languagesText'
  | 'userAgent'
  | 'labelsText';

export type SessionCreateValidationResult = {
  readonly valid: boolean;
  readonly errors: readonly string[];
  readonly fieldErrors: Partial<Record<SessionCreateValidationField, readonly string[]>>;
  readonly request: CreateSessionRequest | null;
  readonly preview: string;
};

export function createNewSessionCreateDraft(): SessionCreateDraft {
  return {
    projectId: '',
    templateId: '',
    ownerMode: '',
    browserContextMode: '',
    browserContextId: '',
    egressProfileId: '',
    idleTimeoutSec: '',
    viewportWidth: '',
    viewportHeight: '',
    capabilityBrowserInput: DEFAULT_CAPABILITIES.browser_input,
    capabilityClipboard: DEFAULT_CAPABILITIES.clipboard,
    capabilityAudio: DEFAULT_CAPABILITIES.audio,
    capabilityMicrophone: DEFAULT_CAPABILITIES.microphone,
    capabilityCamera: DEFAULT_CAPABILITIES.camera,
    capabilityFileTransfer: DEFAULT_CAPABILITIES.file_transfer,
    capabilityResize: DEFAULT_CAPABILITIES.resize,
    recordingEnabled: false,
    recordingRetentionSec: '',
    locale: '',
    languagesText: '',
    timezone: '',
    browserIdentity: '',
    userAgent: '',
    labelsText: '',
  };
}

export function hasSessionCreateDraftChanges(draft: SessionCreateDraft): boolean {
  const initial = createNewSessionCreateDraft();
  return JSON.stringify(normalizedDraft(draft)) !== JSON.stringify(normalizedDraft(initial));
}

export function validateSessionCreateDraft(
  draft: SessionCreateDraft,
  options: SessionCreateOptions = emptySessionCreateOptions(),
): SessionCreateValidationResult {
  const errors: string[] = [];
  const fieldErrors: Partial<Record<SessionCreateValidationField, string[]>> = {};
  const addError = (field: SessionCreateValidationField, message: string): void => {
    errors.push(message);
    fieldErrors[field] = [...(fieldErrors[field] ?? []), message];
  };

  const projectId = draft.projectId.trim();
  const project = options.projects.find((candidate) => candidate.id === projectId) ?? null;
  if (projectId) {
    if (options.projects.length > 0 && !project) {
      addError('projectId', 'Selected project is not available.');
    } else if (project?.state === 'archived') {
      addError('projectId', 'Archived projects cannot be used for new sessions.');
    }
  }

  const templateId = draft.templateId.trim();
  if (templateId) {
    validateGovernedOptionSelection(
      'templateId',
      templateId,
      'session_template',
      project,
      options.sessionTemplates,
      'Selected session template is not available.',
      addError,
    );
  }

  const browserContextId = draft.browserContextId.trim();
  if (draft.browserContextMode === 'reusable') {
    if (!browserContextId) {
      addError('browserContextId', 'Reusable browser context requires a selected context.');
    } else {
      validateGovernedOptionSelection(
        'browserContextId',
        browserContextId,
        'browser_context',
        project,
        options.browserContexts,
        'Selected browser context is not available.',
        addError,
      );
    }
  } else if (browserContextId) {
    addError('browserContextId', 'Browser context id can only be set for reusable mode.');
  }

  const egressProfileId = draft.egressProfileId.trim();
  if (egressProfileId) {
    validateGovernedOptionSelection(
      'egressProfileId',
      egressProfileId,
      'egress_profile',
      project,
      options.egressProfiles,
      'Selected egress profile is not available.',
      addError,
    );
  }

  const idleTimeoutSec = optionalPositiveInteger(
    draft.idleTimeoutSec,
    'Idle timeout',
    'idleTimeoutSec',
    addError,
  );
  const recordingRetentionSec = draft.recordingEnabled
    ? optionalPositiveInteger(
        draft.recordingRetentionSec,
        'Recording retention',
        'recordingRetentionSec',
        addError,
      )
    : null;
  const viewport = optionalViewport(draft, addError);
  const labels = parseLabels(draft.labelsText, addError);
  const languages = parseLanguages(draft.languagesText, addError);

  const userAgent = draft.userAgent.trim();
  if (/[\r\n]/u.test(userAgent)) {
    addError('userAgent', 'User agent must be a single line.');
  }

  if (errors.length > 0) {
    return {
      valid: false,
      errors,
      fieldErrors,
      request: null,
      preview: 'Fix validation errors before creating the session.',
    };
  }

  const networkIdentity: {
    locale?: string;
    languages?: readonly string[];
    timezone?: string;
    browser_identity?: string;
    user_agent?: string;
    egress_profile_id?: string;
  } = {};
  assignIfPresent(networkIdentity, 'locale', draft.locale.trim());
  if (languages.length > 0) {
    networkIdentity.languages = languages;
  }
  assignIfPresent(networkIdentity, 'timezone', draft.timezone.trim());
  assignIfPresent(networkIdentity, 'browser_identity', draft.browserIdentity.trim());
  assignIfPresent(networkIdentity, 'user_agent', userAgent);
  assignIfPresent(networkIdentity, 'egress_profile_id', egressProfileId);

  const request: {
    project_id?: string;
    template_id?: string;
    browser_context?: { mode: string; context_id?: string };
    network_identity?: typeof networkIdentity;
    owner_mode?: string;
    viewport?: { width: number; height: number };
    capabilities?: SessionCapabilities;
    idle_timeout_sec?: number;
    labels?: Readonly<Record<string, string>>;
    recording?: SessionRecordingPolicy;
  } = {};
  assignIfPresent(request, 'project_id', projectId);
  assignIfPresent(request, 'template_id', templateId);
  assignIfPresent(request, 'owner_mode', draft.ownerMode);
  if (draft.browserContextMode === 'fresh' || draft.browserContextMode === 'ephemeral') {
    request.browser_context = { mode: draft.browserContextMode };
  } else if (draft.browserContextMode === 'reusable') {
    request.browser_context = { mode: 'reusable', context_id: browserContextId };
  }
  if (idleTimeoutSec !== null) {
    request.idle_timeout_sec = idleTimeoutSec;
  }
  if (viewport) {
    request.viewport = viewport;
  }
  const capabilities = capabilitiesFromDraft(draft);
  if (!sessionCapabilitiesEqual(capabilities, DEFAULT_CAPABILITIES)) {
    request.capabilities = capabilities;
  }
  if (Object.keys(labels).length > 0) {
    request.labels = labels;
  }
  if (Object.keys(networkIdentity).length > 0) {
    request.network_identity = networkIdentity;
  }
  if (draft.recordingEnabled) {
    request.recording = {
      mode: 'always',
      format: 'webm',
      ...(recordingRetentionSec !== null ? { retention_sec: recordingRetentionSec } : {}),
    };
  }

  return {
    valid: true,
    errors: [],
    fieldErrors: {},
    request,
    preview: JSON.stringify(request, null, 2),
  };
}

export function emptySessionCreateOptions(): SessionCreateOptions {
  return {
    projects: [],
    sessionTemplates: [],
    browserContexts: [],
    egressProfiles: [],
  };
}

function capabilitiesFromDraft(draft: SessionCreateDraft): SessionCapabilities {
  return {
    browser_input: draft.capabilityBrowserInput,
    clipboard: draft.capabilityClipboard,
    audio: draft.capabilityAudio,
    microphone: draft.capabilityMicrophone,
    camera: draft.capabilityCamera,
    file_transfer: draft.capabilityFileTransfer,
    resize: draft.capabilityResize,
  };
}

function sessionCapabilitiesEqual(
  left: SessionCapabilities,
  right: SessionCapabilities,
): boolean {
  return left.browser_input === right.browser_input
    && left.clipboard === right.clipboard
    && left.audio === right.audio
    && left.microphone === right.microphone
    && left.camera === right.camera
    && left.file_transfer === right.file_transfer
    && left.resize === right.resize;
}

function validateGovernedOptionSelection(
  field: SessionCreateValidationField,
  selectedId: string,
  kind: ProjectResourceKind,
  project: ProjectResource | null,
  options: readonly ProjectPolicyOption[],
  missingMessage: string,
  addError: (field: SessionCreateValidationField, message: string) => void,
): void {
  const option = options.find((candidate) => candidate.id === selectedId);
  if (options.length > 0 && !option) {
    addError(field, missingMessage);
    return;
  }
  if (option) {
    const decision = new SessionCreateGovernancePresenter().evaluateResource(project, kind, option);
    if (!decision.allowed) {
      addError(field, decision.reason);
    }
  }
}

function optionalPositiveInteger(
  value: string,
  label: string,
  field: SessionCreateValidationField,
  addError: (field: SessionCreateValidationField, message: string) => void,
): number | null {
  const trimmed = normalizedText(value);
  if (!trimmed) {
    return null;
  }
  const parsed = Number(trimmed);
  if (!Number.isSafeInteger(parsed) || parsed < 1) {
    addError(field, `${label} must be a positive whole number.`);
    return null;
  }
  return parsed;
}

function optionalViewport(
  draft: SessionCreateDraft,
  addError: (field: SessionCreateValidationField, message: string) => void,
): { width: number; height: number } | null {
  const widthText = normalizedText(draft.viewportWidth);
  const heightText = normalizedText(draft.viewportHeight);
  if (!widthText && !heightText) {
    return null;
  }
  if (!widthText || !heightText) {
    addError('viewport', 'Viewport width and height must be provided together.');
    return null;
  }
  const width = Number(widthText);
  const height = Number(heightText);
  if (!Number.isSafeInteger(width) || width < 320 || width > 7680) {
    addError('viewport', 'Viewport width must be a whole number between 320 and 7680.');
  }
  if (!Number.isSafeInteger(height) || height < 240 || height > 4320) {
    addError('viewport', 'Viewport height must be a whole number between 240 and 4320.');
  }
  if (!Number.isSafeInteger(width) || !Number.isSafeInteger(height) || width < 320 || width > 7680 || height < 240 || height > 4320) {
    return null;
  }
  return { width, height };
}

function parseLabels(
  value: string,
  addError: (field: SessionCreateValidationField, message: string) => void,
): Readonly<Record<string, string>> {
  const labels: Record<string, string> = {};
  const seen = new Set<string>();
  for (const line of value.split(/\r?\n/u)) {
    const trimmed = line.trim();
    if (!trimmed) {
      continue;
    }
    const separator = trimmed.indexOf('=');
    if (separator <= 0 || separator === trimmed.length - 1) {
      addError('labelsText', `Label "${trimmed}" must use key=value.`);
      continue;
    }
    const key = trimmed.slice(0, separator).trim();
    const labelValue = trimmed.slice(separator + 1).trim();
    if (seen.has(key)) {
      addError('labelsText', `Label "${key}" is duplicated.`);
      continue;
    }
    seen.add(key);
    labels[key] = labelValue;
  }
  return labels;
}

function parseLanguages(
  value: string,
  addError: (field: SessionCreateValidationField, message: string) => void,
): readonly string[] {
  const languages: string[] = [];
  const seen = new Set<string>();
  for (const entry of value.split(/[,\n]/u)) {
    const language = entry.trim();
    if (!language) {
      continue;
    }
    if (seen.has(language)) {
      addError('languagesText', `Language "${language}" is duplicated.`);
      continue;
    }
    seen.add(language);
    languages.push(language);
  }
  return languages;
}

function assignIfPresent<T extends Record<string, unknown>, K extends keyof T>(
  target: T,
  key: K,
  value: T[K] | '' | null | undefined,
): void {
  if (value !== undefined && value !== null && value !== '') {
      target[key] = value as T[K];
  }
}

function normalizedDraft(draft: SessionCreateDraft): SessionCreateDraft {
  return {
    ...draft,
    projectId: draft.projectId.trim(),
    templateId: draft.templateId.trim(),
    browserContextId: draft.browserContextId.trim(),
    egressProfileId: draft.egressProfileId.trim(),
    idleTimeoutSec: normalizedText(draft.idleTimeoutSec),
    recordingRetentionSec: draft.recordingEnabled ? normalizedText(draft.recordingRetentionSec) : '',
    viewportWidth: normalizedText(draft.viewportWidth),
    viewportHeight: normalizedText(draft.viewportHeight),
    locale: draft.locale.trim(),
    languagesText: draft.languagesText.trim(),
    timezone: draft.timezone.trim(),
    browserIdentity: draft.browserIdentity.trim(),
    userAgent: draft.userAgent.trim(),
    labelsText: draft.labelsText.trim(),
  };
}

function normalizedText(value: string): string {
  return String(value ?? '').trim();
}
