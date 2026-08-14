import assert from 'node:assert/strict';
import test from 'node:test';

import run from '../../dev/workflows/single-node-qualification/run.mjs';

test('qualification workflow visits the target and uploads retained evidence', async () => {
  const calls = [];
  const page = {
    goto: async (...args) => calls.push(['goto', ...args]),
    locator: () => ({ innerText: async () => 'ready' }),
    url: () => 'http://web:8080/healthz',
  };
  const artifacts = {
    uploadTextFile: async (request) => {
      calls.push(['upload', request]);
      return { file_id: 'file-1' };
    },
  };

  const result = await run({
    page,
    artifacts,
    input: {
      target_url: 'http://web:8080/healthz',
      output_workspace_id: 'workspace-1',
    },
  });

  assert.equal(calls[0][0], 'goto');
  assert.equal(calls[0][1], 'http://web:8080/healthz');
  assert.equal(calls[1][0], 'upload');
  assert.equal(calls[1][1].workspaceId, 'workspace-1');
  assert.match(calls[1][1].text, /body=ready/u);
  assert.deepEqual(result, {
    body: 'ready',
    final_url: 'http://web:8080/healthz',
    output_file_id: 'file-1',
  });
});
