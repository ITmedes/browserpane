import { afterEach, describe, expect, it } from 'vitest';

import { buildIdentityReviewViewModel } from '$lib/identity/identity-review-view-model';
import { identityAccessReviewFixture } from '$lib/test-utils/identity-fixtures';
import { byTestId, cleanupRenderedComponents, renderComponent } from '$lib/test-utils/svelte-component-test';
import IdentityReviewSummary from './IdentityReviewSummary.svelte';

afterEach(cleanupRenderedComponents);

describe('IdentityReviewSummary', () => {
  it('renders sanitized principal, project, delegation, and unmapped evidence', () => {
    const target = renderComponent(IdentityReviewSummary, {
      model: buildIdentityReviewViewModel(identityAccessReviewFixture()),
    });

    expect(byTestId(target, 'identity-principal-name').textContent).toContain('Demo Operator');
    expect(byTestId(target, 'identity-metric-sessions').textContent).toContain('1 / 2');
    expect(byTestId(target, 'identity-project-access').textContent).toContain('Operations');
    expect(byTestId(target, 'identity-delegations').textContent).toContain('bpane-mcp-bridge');
    expect(byTestId(target, 'identity-unmapped-signals').textContent).toContain('Safe group signal');
    expect(byTestId(target, 'identity-enforcement-boundary').textContent).toContain('not complete RBAC');
    expect(target.textContent).not.toContain('Bearer');
    expect(target.textContent).not.toContain('client_secret');
  });

  it('renders explicit empty states', () => {
    const review = identityAccessReviewFixture();
    const target = renderComponent(IdentityReviewSummary, {
      model: buildIdentityReviewViewModel({
        ...review,
        projects: [],
        delegated_principals: [],
        unmapped_principal_signals: [],
      }),
    });

    expect(byTestId(target, 'identity-project-access').textContent).toContain('No projects are visible');
    expect(byTestId(target, 'identity-delegations').textContent).toContain('No automation principals');
    expect(byTestId(target, 'identity-unmapped-signals').textContent).toContain('No unmapped safe');
  });
});
