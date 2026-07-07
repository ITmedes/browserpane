import { describe, expect, it, vi } from 'vitest';

import type { WorkflowDefinitionResource } from './workflow-types';
import {
  ADMIN_HIDDEN_LABEL,
  ADMIN_TEMPLATE_LABEL,
  BROWSERPANE_TOUR_TEMPLATE,
  INCLUDE_HIDDEN_STORAGE_KEY,
  hiddenWorkflowDefinitions,
  includeHiddenWorkflowDefinitions,
  isBrowserPaneTourDefinition,
  visibleWorkflowDefinitions,
  workflowDefinitionKind,
} from './workflow-visibility';

describe('workflow visibility', () => {
  it('prioritizes templates and hides smoke/internal definitions', () => {
    const visible = visibleWorkflowDefinitions([
      workflow({ id: 'smoke', name: 'smoke workflow', latest: 'v1' }),
      workflow({ id: 'plain', name: 'Customer workflow', latest: 'v1' }),
      workflow({ id: 'tour', name: 'BrowserPane Tour', latest: 'v1', template: BROWSERPANE_TOUR_TEMPLATE }),
      workflow({ id: 'hidden', name: 'Hidden workflow', latest: 'v1', hidden: true }),
      workflow({ id: 'unpublished-template', name: 'Template without version', template: 'draft' }),
    ]);

    expect(visible.map((definition) => definition.id)).toEqual(['tour', 'plain']);
    expect(hiddenWorkflowDefinitions([
      workflow({ id: 'smoke', name: 'smoke workflow', latest: 'v1' }),
      workflow({ id: 'plain', name: 'Customer workflow', latest: 'v1' }),
    ]).map((definition) => definition.id)).toEqual(['smoke']);
  });

  it('classifies templates and reads the hidden include flag defensively', () => {
    const tour = workflow({ id: 'tour', name: 'BrowserPane Tour', latest: 'v1', template: BROWSERPANE_TOUR_TEMPLATE });
    vi.stubGlobal('sessionStorage', {
      getItem: (key: string) => key === INCLUDE_HIDDEN_STORAGE_KEY ? 'true' : null,
    });

    expect(isBrowserPaneTourDefinition(tour)).toBe(true);
    expect(workflowDefinitionKind(tour)).toBe('Example template');
    expect(includeHiddenWorkflowDefinitions()).toBe(true);

    vi.unstubAllGlobals();
  });
});

function workflow(options: {
  readonly id: string;
  readonly name: string;
  readonly latest?: string | null;
  readonly template?: string;
  readonly hidden?: boolean;
}): WorkflowDefinitionResource {
  return {
    id: options.id,
    name: options.name,
    description: null,
    labels: {
      ...(options.template ? { [ADMIN_TEMPLATE_LABEL]: options.template } : {}),
      ...(options.hidden ? { [ADMIN_HIDDEN_LABEL]: 'true' } : {}),
      ...(options.id === 'smoke' ? { suite: 'admin-workflow-smoke' } : {}),
    },
    latest_version: options.latest ?? null,
    created_at: '2026-06-21T09:00:00.000Z',
    updated_at: '2026-06-21T10:00:00.000Z',
  };
}
