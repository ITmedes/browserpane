import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import { sessionResource } from '$lib/test-utils/session-fixtures';
import { workflowRunFixture } from '$lib/test-utils/workflow-run-fixture';
import ProjectRelatedWorkEvidence from './ProjectRelatedWorkEvidence.svelte';

afterEach(cleanupRenderedComponents);

describe('ProjectRelatedWorkEvidence', () => {
  it('renders project work and delegates refresh', () => {
    const onRefresh = vi.fn();
    const target = renderComponent(ProjectRelatedWorkEvidence, {
      projectId: 'project-1',
      sessionsState: { status: 'ready', sessions: [sessionResource({ queued: true })] },
      workflowRunsState: { status: 'ready', runs: [workflowRunFixture()] },
      onRefresh,
    });

    expect(byTestId(target, 'project-related-sessions').textContent).toContain('Queue: #2');
    expect(byTestId(target, 'project-related-workflow-runs').textContent).toContain('run-1');
    byTestId(target, 'project-related-work-refresh').click();
    expect(onRefresh).toHaveBeenCalledOnce();
  });

  it('shows one catalog error without hiding successful related work', () => {
    const target = renderComponent(ProjectRelatedWorkEvidence, {
      projectId: 'project-1',
      sessionsState: { status: 'error', message: 'session catalog unavailable' },
      workflowRunsState: { status: 'ready', runs: [workflowRunFixture()] },
    });

    expect(byTestId(target, 'project-related-sessions').textContent).toContain(
      'session catalog unavailable',
    );
    expect(byTestId(target, 'project-related-workflow-runs').textContent).toContain('run-1');
  });
});
