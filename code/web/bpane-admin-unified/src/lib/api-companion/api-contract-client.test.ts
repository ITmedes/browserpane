import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it, vi } from 'vitest';
import {
  ApiContractClient,
  parseApiContractEvidence,
  parseClassificationCatalog,
  parseCompatibilityCatalog,
  parseExampleCatalog,
  parseOperationCatalog,
} from './api-contract-client';

const OPERATIONS = {
  version: 1,
  contract: 'bpane-control-v1',
  operations: [
    operation('createProject', 'POST', '/api/v1/projects', 'Projects', 'owner-bearer', 'ui-primary', ['201', '401']),
    operation('createSession', 'POST', '/api/v1/sessions', 'Sessions', 'owner-bearer', 'ui-primary', ['201', '401']),
    operation('issueSessionAccessToken', 'POST', '/api/v1/sessions/{session_id}/access-tokens', 'Session Runtime', 'owner-bearer', 'ui-evidence', ['200', '401']),
    operation('appendWorkflowRunLog', 'POST', '/api/v1/workflow-runs/{run_id}/logs', 'Workflows', 'session-automation', 'internal-worker', ['200', '401']),
    operation('finalizeRecording', 'POST', '/api/v1/sessions/{session_id}/recordings/{recording_id}/finalize', 'Session Recordings', 'recording-worker', 'internal-worker', ['200', '401']),
  ],
};

const CLASSIFICATIONS = {
  version: 1,
  contract: 'bpane-control-v1',
  classifications: {
    'ui-primary': ['createProject', 'createSession'],
    'ui-evidence': ['issueSessionAccessToken'],
    'api-companion': [],
    'internal-worker': ['appendWorkflowRunLog', 'finalizeRecording'],
  },
};

const EXAMPLES = {
  version: 1,
  contract: 'bpane-control-v1',
  examples: [{
    name: 'companion-project-create',
    operationId: 'createProject',
    request: { method: 'POST', path: '/api/v1/projects', body: { name: 'Pilot' } },
    response: { status: 401, body: { error: 'missing bearer token' } },
  }],
};

const COMPATIBILITY = {
  version: 1,
  contract: 'bpane-control-v1',
  surfaces: [{
    id: 'legacy-session-status',
    family: 'Gateway legacy',
    methods: ['GET'],
    path: '/api/session/status',
    auth: 'owner-bearer',
    stability: 'legacy',
    purpose: 'Compatibility status.',
  }],
};

describe('ApiContractClient', () => {
  it('loads and cross-validates all canonical evidence documents', async () => {
    const payloads = [OPERATIONS, CLASSIFICATIONS, EXAMPLES, COMPATIBILITY];
    const fetchImpl = vi.fn<typeof fetch>().mockImplementation(async () =>
      new Response(JSON.stringify(payloads.shift()), { status: 200, headers: { 'content-type': 'application/json' } }));

    const evidence = await new ApiContractClient({ baseUrl: 'https://pane.example', fetchImpl }).load();

    expect(evidence.operations).toHaveLength(5);
    expect(evidence.examples[0]?.request.body).toEqual({ name: 'Pilot' });
    expect(evidence.compatibilitySurfaces[0]?.id).toBe('legacy-session-status');
    expect(fetchImpl.mock.calls.map(([url]) => String(url))).toEqual([
      'https://pane.example/openapi/bpane-control-v1.operations.json',
      'https://pane.example/openapi/bpane-control-v1.classifications.json',
      'https://pane.example/openapi/bpane-control-v1.examples.json',
      'https://pane.example/openapi/bpane-control-v1.compatibility.json',
    ]);
  });

  it('reports unavailable and malformed evidence with the affected path', async () => {
    const unavailable = vi.fn<typeof fetch>().mockResolvedValue(new Response('', { status: 503 }));
    await expect(new ApiContractClient({ baseUrl: 'https://pane.example', fetchImpl: unavailable }).load())
      .rejects.toThrow('HTTP 503 for /openapi/bpane-control-v1.operations.json');

    const malformed = vi.fn<typeof fetch>().mockResolvedValue(new Response('{', { status: 200 }));
    await expect(new ApiContractClient({ baseUrl: 'https://pane.example', fetchImpl: malformed }).load())
      .rejects.toThrow('not valid JSON');
  });
});

describe('API contract evidence parsers', () => {
  it('accepts the committed repository evidence', () => {
    const root = path.resolve(import.meta.dirname, '../../../../../..');
    const readEvidence = (name: string): unknown => JSON.parse(fs.readFileSync(
      path.join(root, 'openapi', `bpane-control-v1.${name}.json`),
      'utf8',
    ));

    const evidence = parseApiContractEvidence(
      readEvidence('operations'),
      readEvidence('classifications'),
      readEvidence('examples'),
      readEvidence('compatibility'),
    );

    expect(evidence.operations).toHaveLength(144);
    expect(evidence.operations.filter(({ auth }) => auth === 'recording-worker')).toHaveLength(2);
    expect(evidence.operations.filter(({ auth }) => auth === 'machine-bearer')).toHaveLength(4);
  });

  it('returns safe, joined evidence', () => {
    const evidence = parseApiContractEvidence(OPERATIONS, CLASSIFICATIONS, EXAMPLES, COMPATIBILITY);
    expect(evidence.operations.map((item) => item.operationId)).toEqual([
      'createProject',
      'createSession',
      'issueSessionAccessToken',
      'appendWorkflowRunLog',
      'finalizeRecording',
    ]);
  });

  it('rejects unsupported catalog metadata and collection shapes', () => {
    expect(() => parseOperationCatalog({ ...OPERATIONS, version: 2 })).toThrow('format version 1');
    expect(() => parseClassificationCatalog({ ...CLASSIFICATIONS, classifications: [] })).toThrow('must be an object');
    expect(() => parseExampleCatalog({ ...EXAMPLES, examples: null })).toThrow('must be an array');
    expect(() => parseCompatibilityCatalog({ ...COMPATIBILITY, surfaces: null })).toThrow('must be an array');
  });

  it('rejects duplicate or malformed operations', () => {
    expect(() => parseOperationCatalog({
      ...OPERATIONS,
      operations: [OPERATIONS.operations[0], OPERATIONS.operations[0]],
    })).toThrow('Duplicate API operation id');
    expect(() => parseOperationCatalog({
      ...OPERATIONS,
      operations: [{ ...OPERATIONS.operations[0], method: 'PATCH' }],
    })).toThrow('method is unsupported');
    expect(() => parseOperationCatalog({
      ...OPERATIONS,
      operations: [{ ...OPERATIONS.operations[0], path: '/api/session' }],
    })).toThrow('must start with /api/v1/');
    expect(() => parseOperationCatalog({
      ...OPERATIONS,
      operations: [{ ...OPERATIONS.operations[0], responses: ['999'] }],
    })).toThrow('unsupported status');
  });

  it('rejects classification omissions, drift, duplicates, and stale ids', () => {
    expect(() => parseApiContractEvidence(OPERATIONS, {
      ...CLASSIFICATIONS,
      classifications: { ...CLASSIFICATIONS.classifications, 'ui-primary': ['createProject'] },
    }, EXAMPLES, COMPATIBILITY)).toThrow('missing from classifications: createSession');

    expect(() => parseApiContractEvidence(OPERATIONS, {
      ...CLASSIFICATIONS,
      classifications: {
        ...CLASSIFICATIONS.classifications,
        'ui-primary': ['createSession'],
        'api-companion': ['createProject'],
      },
    }, EXAMPLES, COMPATIBILITY)).toThrow('classification drift: createProject');

    expect(() => parseApiContractEvidence(OPERATIONS, {
      ...CLASSIFICATIONS,
      classifications: {
        ...CLASSIFICATIONS.classifications,
        'api-companion': ['createProject'],
      },
    }, EXAMPLES, COMPATIBILITY)).toThrow('multiple classifications');

    expect(() => parseApiContractEvidence(OPERATIONS, {
      ...CLASSIFICATIONS,
      classifications: {
        ...CLASSIFICATIONS.classifications,
        'api-companion': ['removedOperation'],
      },
    }, EXAMPLES, COMPATIBILITY)).toThrow('unknown operations: removedOperation');
  });

  it('rejects examples that drift, use undeclared responses, or contain secret fields', () => {
    expect(() => parseApiContractEvidence(OPERATIONS, CLASSIFICATIONS, {
      ...EXAMPLES,
      examples: [{ ...EXAMPLES.examples[0], operationId: 'missingOperation' }],
    }, COMPATIBILITY)).toThrow('unknown operation');
    expect(() => parseApiContractEvidence(OPERATIONS, CLASSIFICATIONS, {
      ...EXAMPLES,
      examples: [{ ...EXAMPLES.examples[0], request: { method: 'GET', path: '/api/v1/projects' } }],
    }, COMPATIBILITY)).toThrow('does not match operation');
    expect(() => parseApiContractEvidence(OPERATIONS, CLASSIFICATIONS, {
      ...EXAMPLES,
      examples: [{ ...EXAMPLES.examples[0], response: { status: 418 } }],
    }, COMPATIBILITY)).toThrow('response is not declared');
    expect(() => parseExampleCatalog({
      ...EXAMPLES,
      examples: [{
        ...EXAMPLES.examples[0],
        request: { ...EXAMPLES.examples[0]!.request, body: { client_secret: 'unsafe' } },
      }],
    })).toThrow('contains sensitive field client_secret');
  });

  it('rejects malformed compatibility surfaces and frozen collisions', () => {
    expect(() => parseCompatibilityCatalog({
      ...COMPATIBILITY,
      surfaces: [{ ...COMPATIBILITY.surfaces[0], methods: ['TRACE'] }],
    })).toThrow('method is unsupported');
    expect(() => parseCompatibilityCatalog({
      ...COMPATIBILITY,
      surfaces: [{ ...COMPATIBILITY.surfaces[0], auth: 'browser-token' }],
    })).toThrow('auth is unsupported');
    expect(() => parseApiContractEvidence(OPERATIONS, CLASSIFICATIONS, EXAMPLES, {
      ...COMPATIBILITY,
      surfaces: [{ ...COMPATIBILITY.surfaces[0], methods: ['POST'], path: '/api/v1/projects' }],
    })).toThrow('collides with frozen API operation');
  });

  it('accepts concrete example ids for templated operation paths', () => {
    const examples = {
      ...EXAMPLES,
      examples: [{
        name: 'companion-session-connect-ticket',
        operationId: 'issueSessionAccessToken',
        request: {
          method: 'POST',
          path: '/api/v1/sessions/22222222-2222-4222-8222-222222222222/access-tokens',
        },
        response: { status: 401, body: { error: 'missing bearer token' } },
      }],
    };
    expect(() => parseApiContractEvidence(OPERATIONS, CLASSIFICATIONS, examples, COMPATIBILITY)).not.toThrow();
  });
});

function operation(
  operationId: string,
  method: string,
  path: string,
  tag: string,
  auth: string,
  classification: string,
  responses: readonly string[],
): Record<string, unknown> {
  return { operationId, method, path, tags: [tag], auth, classification, responses };
}
