import { afterEach, describe, expect, it } from 'vitest';
import { apiContractEvidenceFixture } from '$lib/test-utils/api-contract-fixtures';
import { byTestId, cleanupRenderedComponents, renderComponent } from '$lib/test-utils/svelte-component-test';
import ApiContractSummary from './ApiContractSummary.svelte';

afterEach(cleanupRenderedComponents);

describe('ApiContractSummary', () => {
  it('renders exact generated ownership counts', () => {
    const target = renderComponent(ApiContractSummary, { operations: apiContractEvidenceFixture().operations });
    expect(byTestId(target, 'api-summary-total').textContent).toContain('8');
    expect(byTestId(target, 'api-summary-ui-primary').textContent).toContain('4');
    expect(byTestId(target, 'api-summary-ui-evidence').textContent).toContain('2');
    expect(byTestId(target, 'api-summary-api-companion').textContent).toContain('1');
    expect(byTestId(target, 'api-summary-internal-worker').textContent).toContain('1');
  });
});
