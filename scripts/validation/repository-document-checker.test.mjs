import assert from 'node:assert/strict';
import test from 'node:test';

import { RepositoryDocumentChecker } from './repository-document-checker.mjs';

test('repository document checker composes Markdown, YAML, and workflow checks', () => {
  const calls = [];
  const checker = new RepositoryDocumentChecker('/repo', {
    markdownChecker: {
      check(path) {
        calls.push(['markdown', path]);
        return path === 'broken.md' ? ['broken Markdown'] : [];
      }
    },
    yamlParser: {
      parse(path) {
        calls.push(['yaml', path]);
        if (path.endsWith('broken.yml')) throw new Error('bad YAML');
        return { parsed: true };
      }
    },
    workflowChecker: {
      check(path, workflow) {
        calls.push(['workflow', path, workflow.parsed]);
        return ['workflow policy'];
      }
    }
  });

  const errors = checker.check({
    markdownFiles: ['good.md', 'broken.md'],
    yamlFiles: ['config.yml', '.github/workflows/check.yml', 'broken.yml'],
    workflowFiles: ['.github/workflows/check.yml']
  });

  assert.deepEqual(errors, ['broken Markdown', 'broken.yml is invalid YAML: bad YAML',
    'workflow policy']);
  assert.ok(calls.some((call) => call[0] === 'workflow' && call[2] === true));
});
