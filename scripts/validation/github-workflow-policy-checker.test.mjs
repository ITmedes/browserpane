import assert from 'node:assert/strict';
import test from 'node:test';

import { GitHubWorkflowPolicyChecker } from './github-workflow-policy-checker.mjs';

const SHA = '0123456789abcdef0123456789abcdef01234567';

test('workflow policy accepts pinned least-privilege jobs and bounded artifacts', () => {
  const workflow = {
    on: { pull_request: {} },
    permissions: { contents: 'read' },
    jobs: {
      validate: {
        'runs-on': 'ubuntu-24.04',
        'timeout-minutes': 20,
        steps: [
          { uses: `actions/checkout@${SHA}`, with: { 'persist-credentials': false } },
          { uses: `actions/setup-node@${SHA}`, with: {
            cache: 'npm', 'cache-dependency-path': 'package-lock.json'
          } },
          { uses: `actions/cache@${SHA}`, with: { key: "cargo-${{ hashFiles('Cargo.lock') }}" } },
          { uses: `actions/upload-artifact@${SHA}`, with: {
            path: 'test-results/coverage/rust.md', 'retention-days': 7
          } }
        ]
      }
    }
  };

  assert.deepEqual(new GitHubWorkflowPolicyChecker().check('workflow.yml', workflow), []);
});

test('workflow policy rejects mutable, privileged, unbounded, and secret-bearing jobs', () => {
  const workflow = {
    on: { pull_request_target: {} },
    permissions: { contents: 'write' },
    jobs: {
      unsafe: {
        'runs-on': 'ubuntu-latest',
        steps: [
          { uses: 'actions/checkout@v4' },
          { uses: `actions/upload-artifact@${SHA}`, with: {
            path: 'private/browser-profile', 'retention-days': 30
          } },
          { run: '${{ secrets.PRODUCTION_TOKEN }}' }
        ]
      }
    }
  };

  const errors = new GitHubWorkflowPolicyChecker().check('workflow.yml', workflow);

  assert.ok(errors.length >= 7);
  assert.ok(errors.some((error) => error.includes('pull_request_target')));
  assert.ok(errors.some((error) => error.includes('must not reference')));
});

test('workflow policy permits only guarded main-branch package publication', () => {
  const publish = {
    on: { pull_request: {}, push: { branches: ['main'] }, workflow_dispatch: {} },
    permissions: { contents: 'read' },
    jobs: {
      publish: {
        if: "github.event_name != 'pull_request'",
        permissions: { contents: 'read', packages: 'write' },
        'runs-on': 'ubuntu-24.04',
        'timeout-minutes': 30,
        steps: [],
      },
    },
  };
  assert.deepEqual(new GitHubWorkflowPolicyChecker().check('builder.yml', publish), []);

  publish.jobs.publish.if = undefined;
  const errors = new GitHubWorkflowPolicyChecker().check('builder.yml', publish);
  assert.ok(errors.some((error) => error.includes('root permissions must be exactly')));
});
