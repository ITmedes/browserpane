import fs from 'node:fs';

import { validateComposeNamespace } from './compose-namespace.mjs';

const CI_LABEL = 'bpane_ci_namespace';
const RESOURCE_ID_PATTERN = /^[a-zA-Z0-9][a-zA-Z0-9._:-]{0,127}$/;
const MUTATING_METHODS = new Set(['POST', 'PUT', 'PATCH']);
const ROOT_LABEL_COLLECTIONS = new Set([
  'automation-tasks',
  'browser-contexts',
  'credential-bindings',
  'egress-profiles',
  'extensions',
  'file-workspaces',
  'identity-mappings',
  'projects',
  'service-principals',
  'session-templates',
  'sessions',
  'workflow-endpoints',
  'workflow-runs',
  'workflows',
]);

export class ComposeResourceRegistry {
  #filePath;
  #namespace;

  constructor(filePath, namespace) {
    if (typeof filePath !== 'string' || filePath.length === 0) {
      throw new TypeError('Compose resource registry path is required.');
    }
    this.#filePath = filePath;
    this.#namespace = validateComposeNamespace(namespace);
  }

  record(kind, id) {
    if (!/^[a-z][a-z0-9_]{0,31}$/u.test(kind) || !RESOURCE_ID_PATTERN.test(id)) return;
    fs.appendFileSync(this.#filePath, `${JSON.stringify({
      namespace: this.#namespace,
      kind,
      id,
    })}\n`, { encoding: 'utf8', mode: 0o600 });
  }

  load() {
    if (!fs.existsSync(this.#filePath)) return [];
    const records = fs.readFileSync(this.#filePath, 'utf8').split('\n').filter(Boolean).map((line) => {
      const record = JSON.parse(line);
      if (record.namespace !== this.#namespace
        || !/^[a-z][a-z0-9_]{0,31}$/u.test(record.kind ?? '')
        || !RESOURCE_ID_PATTERN.test(record.id ?? '')) {
        throw new Error('Compose resource registry contains an invalid or foreign record.');
      }
      return Object.freeze(record);
    });
    return [...new Map(records.map((record) => [`${record.kind}:${record.id}`, record])).values()];
  }
}

export function namespaceApiPayload(url, method, payload, namespace) {
  if (!namespace || !MUTATING_METHODS.has(String(method ?? 'GET').toUpperCase())) return payload;
  validateComposeNamespace(namespace);
  if (!isControlApiUrl(url) || !payload || typeof payload !== 'object' || Array.isArray(payload)) {
    return payload;
  }
  const copy = structuredClone(payload);
  addNamespaceToExistingLabels(copy, namespace);
  if (acceptsRootLabels(url) && (!copy.labels || typeof copy.labels !== 'object')) {
    copy.labels = { [CI_LABEL]: namespace };
  }
  return copy;
}

export function resourceRecords(url, responseBody) {
  if (!isControlApiUrl(url) || !responseBody || typeof responseBody !== 'object') return [];
  const kind = resourceKind(url);
  const records = [];
  if (kind && typeof responseBody.id === 'string') records.push({ kind, id: responseBody.id });
  if (kind === 'workflow_run' && typeof responseBody.session_id === 'string') {
    records.push({ kind: 'session', id: responseBody.session_id });
  }
  const sessionId = sessionIdFromUrl(url);
  if (sessionId && kind === 'recording') records.push({ kind: 'session', id: sessionId });
  return records;
}

function addNamespaceToExistingLabels(value, namespace) {
  if (!value || typeof value !== 'object') return;
  if (!Array.isArray(value) && value.labels && typeof value.labels === 'object'
    && !Array.isArray(value.labels)) {
    value.labels[CI_LABEL] = namespace;
  }
  for (const child of Object.values(value)) addNamespaceToExistingLabels(child, namespace);
}

function acceptsRootLabels(url) {
  const segments = apiSegments(url);
  return segments.length === 1 && ROOT_LABEL_COLLECTIONS.has(segments[0]);
}

function resourceKind(url) {
  const segments = apiSegments(url);
  if (segments.length === 0) return null;
  if (segments[0] === 'sessions' && segments.length === 3 && segments[2] === 'recordings') {
    return 'recording';
  }
  if (segments.length !== 1) return null;
  const singular = {
    'automation-tasks': 'automation_task',
    'browser-contexts': 'browser_context',
    'credential-bindings': 'credential_binding',
    'egress-profiles': 'egress_profile',
    extensions: 'extension',
    'file-workspaces': 'file_workspace',
    'identity-mappings': 'identity_mapping',
    projects: 'project',
    'service-principals': 'service_principal',
    'session-templates': 'session_template',
    sessions: 'session',
    'workflow-endpoints': 'workflow_endpoint',
    'workflow-event-subscriptions': 'workflow_event_subscription',
    'workflow-runs': 'workflow_run',
    workflows: 'workflow',
  };
  return singular[segments[0]] ?? null;
}

function sessionIdFromUrl(url) {
  const segments = apiSegments(url);
  return segments[0] === 'sessions' && typeof segments[1] === 'string' ? segments[1] : null;
}

function isControlApiUrl(url) {
  try {
    return new URL(url, 'http://localhost').pathname.startsWith('/api/v1/');
  } catch {
    return false;
  }
}

function apiSegments(url) {
  try {
    const pathname = new URL(url, 'http://localhost').pathname;
    if (!pathname.startsWith('/api/v1/')) return [];
    return pathname.slice('/api/v1/'.length).split('/').filter(Boolean);
  } catch {
    return [];
  }
}
