import { afterEach, describe, expect, it, vi } from 'vitest';
import { apiContractEvidenceFixture } from '$lib/test-utils/api-contract-fixtures';
import { byTestId, cleanupRenderedComponents, renderComponent } from '$lib/test-utils/svelte-component-test';
import ApiEvidenceRouteTest from './ApiEvidenceRoute.test.svelte';

afterEach(cleanupRenderedComponents);

describe('ApiEvidenceRoute', () => {
  it('renders loading and resolved evidence', async () => {
    let resolveEvidence: ((value: ReturnType<typeof apiContractEvidenceFixture>) => void) | undefined;
    const loadEvidence = vi.fn(() => new Promise<ReturnType<typeof apiContractEvidenceFixture>>((resolve) => {
      resolveEvidence = resolve;
    }));
    const target = renderComponent(ApiEvidenceRouteTest, { loadEvidence });
    expect(byTestId(target, 'api-evidence-loading').textContent).toContain('Loading API contract evidence');
    await vi.waitFor(() => expect(loadEvidence).toHaveBeenCalledOnce());
    resolveEvidence?.(apiContractEvidenceFixture());
    await vi.waitFor(() => expect(byTestId(target, 'api-evidence-ready').textContent).toContain('8 loaded'));
  });

  it('renders errors and retries through the same loader', async () => {
    const loadEvidence = vi.fn()
      .mockRejectedValueOnce(new Error('malformed classification evidence'))
      .mockResolvedValueOnce(apiContractEvidenceFixture());
    const target = renderComponent(ApiEvidenceRouteTest, { loadEvidence });
    await vi.waitFor(() => expect(byTestId(target, 'api-evidence-error').textContent).toContain('malformed'));
    byTestId(target, 'api-evidence-retry').click();
    await vi.waitFor(() => expect(byTestId(target, 'api-evidence-ready').textContent).toContain('8 loaded'));
    expect(loadEvidence).toHaveBeenCalledTimes(2);
  });
});
