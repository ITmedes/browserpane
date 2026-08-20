import { readInput, type PhaseZeroOutput } from './lib/contract.ts';

type WorkflowContext = {
  readonly page: {
    goto(url: string, options: { waitUntil: 'networkidle'; timeout: number }): Promise<void>;
    locator(selector: string): {
      click(options: { timeout: number }): Promise<void>;
      textContent(options: { timeout: number }): Promise<string | null>;
    };
  };
  readonly input: unknown;
};

export default async function run({ page, input }: WorkflowContext): Promise<PhaseZeroOutput> {
  const { scenario } = readInput(input);
  if (scenario === 'runtime_failure') {
    throw new Error('RUNTIME_FAILURE: deterministic fixture failure');
  }
  await page.goto(
    `http://web:8080/workflow-package-fixture.html?scenario=${encodeURIComponent(scenario)}`,
    { waitUntil: 'networkidle', timeout: 30_000 },
  );
  if (scenario === 'authentication_challenge') {
    throw new Error('AUTHENTICATION_CHALLENGE: operator authentication required');
  }
  if (scenario === 'portal_failure') {
    throw new Error('PORTAL_FAILURE: deterministic portal response');
  }
  if (scenario === 'missing_element') {
    try {
      await page.locator('[data-testid="missing-submit"]').click({ timeout: 1_000 });
    } catch {
      throw new Error('MISSING_ELEMENT: submit control was not found');
    }
    throw new Error('MISSING_ELEMENT: submit control unexpectedly existed');
  }
  await page.locator('[data-testid="submit"]').click({ timeout: 5_000 });
  if (scenario === 'ambiguous_post_side_effect') {
    throw new Error('AMBIGUOUS_POST_SIDE_EFFECT: submit acknowledged locally; outcome unknown');
  }
  const receipt = await page
    .locator('[data-testid="receipt"]')
    .textContent({ timeout: 5_000 });
  if (receipt !== 'fixture-receipt-0001') {
    throw new Error('ASSERTION_FAILED: receipt mismatch');
  }
  return { scenario: 'happy_path', status: 'completed', receipt };
}
