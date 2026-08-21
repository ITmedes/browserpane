// @ts-nocheck
import { describe, expect, it } from 'vitest';

import { runWorkflowEndpointConformance } from '../../scripts/run-workflow-endpoint-conformance.mjs';

function response(status, body) {
  return {
    ok: status >= 200 && status < 300,
    status,
    text: async () => JSON.stringify(body),
  };
}

describe('fake BPM workflow endpoint conformance fixture', () => {
  it('proves accepted polling, replay, conflicts, validation, and bounded terminal evidence', async () => {
    const responses = [
      response(202, {
        id: 'invocation-1',
        run_id: 'run-1',
        state: 'pending',
        links: { self_path: '/status/invocation-1' },
      }),
      response(200, {
        id: 'invocation-1',
        run_id: 'run-1',
        state: 'pending',
        links: { self_path: '/status/invocation-1' },
      }),
      response(409, { code: 'idempotency_key_conflict' }),
      response(422, { code: 'input_schema_validation_failed', errors: [] }),
      response(200, {
        id: 'invocation-1',
        run_id: 'run-1',
        state: 'running',
        links: { self_path: '/status/invocation-1' },
      }),
      response(200, {
        id: 'invocation-1',
        run_id: 'run-1',
        state: 'succeeded',
        outcome: { category: 'success', retryable: false },
        side_effect_state: 'confirmed',
        artifacts: [{
          file_id: 'file-1',
          content_path: '/artifacts/file-1/content',
          sha256_hex: 'a'.repeat(64),
          byte_count: 12,
        }],
      }),
    ];
    const calls = [];
    const fetchImpl = async (url, init) => {
      calls.push({ url, init });
      return responses.shift();
    };
    let stdout = '';
    const summary = await runWorkflowEndpointConformance(
      {
        BPANE_ACCESS_TOKEN: 'machine-secret-token',
        BPANE_BASE_URL: 'http://browserpane.example',
        BPANE_BPM_PROJECT_ID: 'project/1',
        BPANE_BPM_IDEMPOTENCY_KEY: 'process-1-activity-1',
        BPANE_BPM_POLL_INTERVAL_MS: '1',
        BPANE_BPM_POLL_TIMEOUT_MS: '100',
      },
      {
        fetchImpl,
        stdout: { write: (value) => { stdout += value; } },
        wait: (resolve) => resolve(),
      },
    );

    expect(summary).toMatchObject({
      ok: true,
      invocation_id: 'invocation-1',
      run_id: 'run-1',
      state: 'succeeded',
      outcome: 'success',
      side_effect_state: 'confirmed',
      artifact_count: 1,
      idempotent_replay_verified: true,
      changed_payload_conflict_verified: true,
      pre_runtime_validation_verified: true,
    });
    expect(calls).toHaveLength(6);
    expect(calls[0].url).toContain('/projects/project%2F1/workflow-endpoints/');
    expect(calls[0].init.headers['Idempotency-Key']).toBe('process-1-activity-1');
    expect(calls[1].init.body).toBe(calls[0].init.body);
    expect(JSON.parse(calls[2].init.body).input.reporting_period).toBe('2026-Q4');
    expect(stdout).not.toContain('machine-secret-token');
    expect(stdout).not.toContain('supplier report');
  });
});
