import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

type ReferencePackage = (context: { page: FixturePage; input: unknown }) => Promise<unknown>;

class FixturePage {
  #receipt = '';

  async goto(): Promise<void> {}

  locator(selector: string): {
    click: () => Promise<void>;
    textContent: () => Promise<string | null>;
  } {
    return {
      click: async () => {
        if (selector.includes('missing-submit')) {
          throw new Error('locator not found');
        }
        this.#receipt = 'fixture-receipt-0001';
      },
      textContent: async () => this.#receipt || null,
    };
  }
}

describe('Phase 0 reference package', () => {
  it('returns deterministic schema-valid happy-path output', async () => {
    const runReferencePackage = await loadReferencePackage();
    const output = await runReferencePackage({
      page: new FixturePage(),
      input: { scenario: 'happy_path' },
    });

    assert.deepEqual(output, {
      scenario: 'happy_path',
      status: 'completed',
      receipt: 'fixture-receipt-0001',
    });
  });

  for (const [scenario, outcome] of [
    ['missing_element', 'MISSING_ELEMENT'],
    ['authentication_challenge', 'AUTHENTICATION_CHALLENGE'],
    ['portal_failure', 'PORTAL_FAILURE'],
    ['runtime_failure', 'RUNTIME_FAILURE'],
    ['ambiguous_post_side_effect', 'AMBIGUOUS_POST_SIDE_EFFECT'],
  ] as const) {
    it(`returns the stable ${scenario} outcome`, async () => {
      const runReferencePackage = await loadReferencePackage();
      await assert.rejects(
        runReferencePackage({ page: new FixturePage(), input: { scenario } }),
        new RegExp(`${outcome}:`, 'u'),
      );
    });
  }

  it('rejects undeclared process variables before browser navigation', async () => {
    const runReferencePackage = await loadReferencePackage();
    await assert.rejects(
      runReferencePackage({ page: new FixturePage(), input: { scenario: 'unknown' } }),
      /^Error: INPUT_VALIDATION:/u,
    );
  });
});

async function loadReferencePackage(): Promise<ReferencePackage> {
  const segments = ['..', '..', '..', '..', 'dev', 'workflows', 'phase0-package', 'run.ts'];
  const module = await import(segments.join('/')) as { default: ReferencePackage };
  return module.default;
}
