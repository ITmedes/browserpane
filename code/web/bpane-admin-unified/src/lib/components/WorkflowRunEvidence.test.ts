import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import {
  workflowRunEventFixture,
  workflowRunLogFixture,
  workflowRunProducedFileFixture,
} from '$lib/test-utils/workflow-run-fixture';
import type { WorkflowRunDetailEvidenceState } from '$lib/workflow-runs/workflow-run-detail-view-model';
import WorkflowRunEvidence from './WorkflowRunEvidence.svelte';

afterEach(cleanupRenderedComponents);

describe('WorkflowRunEvidence', () => {
  it('renders events, logs, produced files, and delegates downloads', () => {
    const onDownloadProducedFile = vi.fn();
    const target = renderComponent(WorkflowRunEvidence, {
      evidence: readyEvidence(),
      onDownloadProducedFile,
    });

    expect(byTestId(target, 'workflow-run-detail-event').textContent).toContain('awaiting_input');
    expect(byTestId(target, 'workflow-run-detail-log').textContent).toContain('Waiting for operator');
    expect(byTestId(target, 'workflow-run-detail-produced-file').textContent).toContain('report.json');
    byTestId(target, 'workflow-run-detail-download-produced-file').click();
    expect(onDownloadProducedFile).toHaveBeenCalledWith(workflowRunProducedFileFixture());
  });

  it('keeps evidence failures local to their sections', () => {
    const target = renderComponent(WorkflowRunEvidence, {
      evidence: {
        events: { status: 'error', message: 'events unavailable' },
        logs: { status: 'ready', items: [workflowRunLogFixture()] },
        producedFiles: { status: 'error', message: 'files unavailable' },
      } satisfies WorkflowRunDetailEvidenceState,
      downloadState: { status: 'error', message: 'artifact expired' },
    });

    expect(byTestId(target, 'workflow-run-detail-events-error').textContent).toContain('events unavailable');
    expect(byTestId(target, 'workflow-run-detail-log').textContent).toContain('Waiting for operator');
    expect(byTestId(target, 'workflow-run-detail-produced-files-error').textContent).toContain('files unavailable');
    expect(byTestId(target, 'workflow-run-detail-download-error').textContent).toContain('artifact expired');
  });
});

function readyEvidence(): WorkflowRunDetailEvidenceState {
  return {
    events: { status: 'ready', items: [workflowRunEventFixture()] },
    logs: { status: 'ready', items: [workflowRunLogFixture()] },
    producedFiles: { status: 'ready', items: [workflowRunProducedFileFixture()] },
  };
}
