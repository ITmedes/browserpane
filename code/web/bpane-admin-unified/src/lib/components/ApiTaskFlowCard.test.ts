import { afterEach, describe, expect, it } from 'vitest';
import { buildApiTaskFlows } from '$lib/api-companion/api-companion-view-model';
import { apiContractEvidenceFixture } from '$lib/test-utils/api-contract-fixtures';
import { byTestId, cleanupRenderedComponents, renderComponent } from '$lib/test-utils/svelte-component-test';
import ApiTaskFlowCard from './ApiTaskFlowCard.svelte';

afterEach(cleanupRenderedComponents);

describe('ApiTaskFlowCard', () => {
  it('renders ordered operations, coverage links, and operator destination', () => {
    const flow = buildApiTaskFlows(apiContractEvidenceFixture())[1]!;
    const target = renderComponent(ApiTaskFlowCard, { flow });
    expect(byTestId(target, 'api-task-sessions-admin-link').getAttribute('href')).toBe('/admin-new/sessions');
    expect(byTestId(target, 'api-task-step-companion-session-create').textContent).toContain('Create session');
    expect(byTestId(target, 'api-task-step-companion-session-connect-ticket').textContent).toContain('Mint connect ticket');
    expect(target.querySelector('a[href="/admin-new/coverage?operation=createSession"]')).toBeInstanceOf(HTMLAnchorElement);
  });
});
