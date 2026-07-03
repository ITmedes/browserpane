import type { CreateWorkflowRunRequest } from '$lib/workflow-runs/workflow-run-types';
import type { WorkflowDefinitionVersionResource } from './workflow-types';

export type WorkflowRunSessionMode = 'version_default' | 'create_session' | 'existing_session';

export type WorkflowInputParameter = {
  readonly name: string;
  readonly label: string;
  readonly kind: 'string' | 'number' | 'boolean' | 'json';
  readonly required: boolean;
  readonly description: string;
  readonly placeholder: string;
  readonly defaultValue: unknown;
};

export type WorkflowRunLaunchValidation =
  | { readonly ok: true; readonly request: CreateWorkflowRunRequest }
  | { readonly ok: false; readonly message: string };

export type WorkflowRunLaunchDraft = {
  readonly workflowId: string;
  readonly version: string;
  readonly inputSchema?: unknown;
  readonly inputText: string;
  readonly sessionMode: WorkflowRunSessionMode;
  readonly existingSessionId: string;
  readonly projectId: string;
};

type JsonSchemaObject = {
  readonly type?: unknown;
  readonly properties?: unknown;
  readonly required?: unknown;
};

export function inputParametersFromSchema(schema: unknown): readonly WorkflowInputParameter[] {
  const object = schemaObject(schema);
  if (!object || object.type !== 'object' || !isRecord(object.properties)) {
    return [];
  }
  const required = new Set(Array.isArray(object.required)
    ? object.required.filter((value): value is string => typeof value === 'string')
    : []);
  return Object.entries(object.properties).map(([name, value]) => {
    const property = isRecord(value) ? value : {};
    const type = typeof property.type === 'string' ? property.type : 'json';
    return {
      name,
      label: humanLabel(name),
      kind: inputKind(type),
      required: required.has(name),
      description: typeof property.description === 'string' ? property.description : '',
      placeholder: placeholderForType(type),
      defaultValue: property.default,
    };
  });
}

export function initialWorkflowInputText(schema: unknown): string {
  const parameters = inputParametersFromSchema(schema);
  if (parameters.length === 0) {
    return '{}';
  }
  const input: Record<string, unknown> = {};
  for (const parameter of parameters) {
    input[parameter.name] = parameter.defaultValue ?? defaultValueForKind(parameter.kind);
  }
  return JSON.stringify(input, null, 2);
}

export function validateWorkflowRunLaunch(draft: WorkflowRunLaunchDraft): WorkflowRunLaunchValidation {
  const workflowId = draft.workflowId.trim();
  const version = draft.version.trim();
  if (!workflowId) {
    return { ok: false, message: 'Workflow id is required.' };
  }
  if (!version) {
    return { ok: false, message: 'Workflow version is required.' };
  }
  const parsed = parseInput(draft.inputText);
  if (!parsed.ok) {
    return parsed;
  }
  const schemaValidation = validateRequiredInputs(draft.inputSchema, parsed.input);
  if (schemaValidation) {
    return { ok: false, message: schemaValidation };
  }
  const projectId = draft.projectId.trim();
  let session: CreateWorkflowRunRequest['session'] | undefined;
  if (draft.sessionMode === 'create_session') {
    session = {
      create_session: projectId
        ? {
            project_id: projectId,
            labels: { origin: 'admin-unified-workflow-run' },
          }
        : {
            labels: { origin: 'admin-unified-workflow-run' },
          },
    };
  } else if (draft.sessionMode === 'existing_session') {
    const existingSessionId = draft.existingSessionId.trim();
    if (!existingSessionId) {
      return { ok: false, message: 'Existing session id is required for this session binding mode.' };
    }
    session = { existing_session_id: existingSessionId };
  }
  return {
    ok: true,
    request: {
      workflow_id: workflowId,
      version,
      input: parsed.input,
      source_system: 'admin_unified',
      source_reference: 'workflow-detail',
      labels: { source: 'admin-unified-workflows' },
      ...(projectId ? { project_id: projectId } : {}),
      ...(session ? { session } : {}),
    },
  };
}

export function workflowRunRequestPreview(draft: WorkflowRunLaunchDraft): string {
  const validation = validateWorkflowRunLaunch(draft);
  if (!validation.ok) {
    return validation.message;
  }
  return JSON.stringify(validation.request, null, 2);
}

export function defaultSessionMode(version: WorkflowDefinitionVersionResource | null): WorkflowRunSessionMode {
  return version?.default_session ? 'version_default' : 'create_session';
}

function parseInput(inputText: string): { readonly ok: true; readonly input: unknown } | { readonly ok: false; readonly message: string } {
  const trimmed = inputText.trim();
  if (!trimmed) {
    return { ok: true, input: {} };
  }
  try {
    return { ok: true, input: JSON.parse(trimmed) };
  } catch (error) {
    return {
      ok: false,
      message: error instanceof Error ? `Input JSON is invalid: ${error.message}` : 'Input JSON is invalid.',
    };
  }
}

function validateRequiredInputs(schema: unknown, input: unknown): string | null {
  const object = schemaObject(schema);
  if (!object || object.type !== 'object' || !Array.isArray(object.required) || object.required.length === 0) {
    return null;
  }
  if (!isRecord(input)) {
    return 'Input must be a JSON object for this workflow version.';
  }
  const missing = object.required
    .filter((value): value is string => typeof value === 'string')
    .filter((name) => input[name] === undefined || input[name] === null || input[name] === '');
  if (missing.length === 0) {
    return null;
  }
  return `Required input parameter${missing.length === 1 ? '' : 's'} missing: ${missing.join(', ')}.`;
}

function schemaObject(schema: unknown): JsonSchemaObject | null {
  if (!isRecord(schema)) {
    return null;
  }
  return schema;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value);
}

function humanLabel(name: string): string {
  return name
    .replace(/[_-]+/g, ' ')
    .replace(/\b\w/g, (character) => character.toUpperCase());
}

function inputKind(type: string): WorkflowInputParameter['kind'] {
  if (type === 'string' || type === 'number' || type === 'integer' || type === 'boolean') {
    return type === 'integer' ? 'number' : type;
  }
  return 'json';
}

function placeholderForType(type: string): string {
  if (type === 'number' || type === 'integer') {
    return '0';
  }
  if (type === 'boolean') {
    return 'false';
  }
  if (type === 'string') {
    return 'value';
  }
  return '{}';
}

function defaultValueForKind(kind: WorkflowInputParameter['kind']): unknown {
  if (kind === 'number') {
    return 0;
  }
  if (kind === 'boolean') {
    return false;
  }
  if (kind === 'json') {
    return {};
  }
  return '';
}
