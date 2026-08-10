import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { ProjectPolicyOption } from '$lib/projects/project-types';
import type { CreateSessionRequest } from '$lib/sessions/session-types';
import { projectResourceFixture } from '$lib/test-utils/project-fixture';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import SessionCreateForm from './SessionCreateForm.svelte';

afterEach(cleanupRenderedComponents);

describe('SessionCreateForm project governance', () => {
  it('shows capacity and disables resources excluded by project policy', async () => {
    const onSave = vi.fn<(request: CreateSessionRequest) => void>();
    const project = projectResourceFixture({
      quotas: { max_active_sessions: 1 },
      policy: { allowed_session_template_ids: ['template-allowed'] },
      usage: {
        active_sessions: 1,
        max_active_sessions: 1,
        alerts: [{
          metric: 'runtime_usage_ms',
          state: 'approaching_limit',
          current_value: 80,
          limit_value: 100,
          threshold_percent: 80,
          message: 'Runtime budget is approaching its limit.',
        }],
      },
    });
    const target = renderComponent(SessionCreateForm, {
      optionsState: {
        status: 'ready',
        options: {
          projects: [project],
          sessionTemplates: [option('template-allowed'), option('template-blocked')],
          browserContexts: [],
          egressProfiles: [],
        },
      },
      onSave,
    });

    setSelectValue(byTestId(target, 'session-create-project-id'), project.id);
    await tick();

    const evidence = byTestId(target, 'session-create-project-governance');
    expect(evidence.textContent).toContain('Active sessions: 1 / 1');
    expect(evidence.textContent).toContain('Runtime budget is approaching its limit');
    const blocked = selectOption(byTestId(target, 'session-create-template-id'), 'template-blocked');
    expect(blocked.disabled).toBe(true);
    expect(blocked.textContent).toContain('unavailable');
    expect(blocked.textContent).toContain('not included');
    expect(onSave).not.toHaveBeenCalled();
  });

  it('keeps a stale selection visible and blocks submit after the project changes', async () => {
    const onSave = vi.fn<(request: CreateSessionRequest) => void>();
    const project = projectResourceFixture({
      policy: { allowed_session_template_ids: ['template-allowed'] },
    });
    const target = renderComponent(SessionCreateForm, {
      optionsState: {
        status: 'ready',
        options: {
          projects: [project],
          sessionTemplates: [option('template-allowed'), option('template-blocked')],
          browserContexts: [],
          egressProfiles: [],
        },
      },
      onSave,
    });

    setSelectValue(byTestId(target, 'session-create-template-id'), 'template-blocked');
    setSelectValue(byTestId(target, 'session-create-project-id'), project.id);
    await tick();

    expect((byTestId(target, 'session-create-template-id') as HTMLSelectElement).value).toBe(
      'template-blocked',
    );
    expect(byTestId(target, 'session-create-template-id-error').textContent).toContain(
      'not included',
    );
    expect((byTestId(target, 'session-create-save') as HTMLButtonElement).disabled).toBe(true);
    byTestId(target, 'session-create-save').click();
    expect(onSave).not.toHaveBeenCalled();
  });
});

function setSelectValue(element: Element, value: string): void {
  (element as HTMLSelectElement).value = value;
  element.dispatchEvent(new Event('change', { bubbles: true }));
}

function selectOption(element: Element, value: string): HTMLOptionElement {
  const optionElement = [...(element as HTMLSelectElement).options]
    .find((candidate) => candidate.value === value);
  expect(optionElement).toBeDefined();
  return optionElement!;
}

function option(id: string): ProjectPolicyOption {
  return {
    id,
    projectId: null,
    name: id,
    description: null,
    state: 'ready',
    scope: 'Owner scoped',
  };
}
