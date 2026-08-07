import assert from 'node:assert/strict';
import { adminRouteUrl, assertNoBodyHorizontalOverflow } from './admin-unified-smoke-lib.mjs';

export async function exerciseExtensionCatalog(page, options, runLabel) {
  const name = `Catalog extension ${runLabel}`;
  await page.goto(adminRouteUrl(options, 'extensions/new'), { waitUntil: 'domcontentloaded' });
  await page.getByTestId('extension-create-form').waitFor({ state: 'visible' });
  await page.getByTestId('extension-create-name').fill(name);
  await page.getByTestId('extension-create-labels').fill('broken-label');
  await page.getByTestId('extension-create-labels-error').waitFor({ state: 'visible' });
  assert.equal(await page.getByTestId('extension-create-submit').isDisabled(), true);
  await page
    .getByTestId('extension-create-labels')
    .fill(`suite=admin-resource-catalogs\nrun=${runLabel}`);
  await page.getByTestId('extension-create-submit').click();
  await page.waitForURL(/\/admin-new\/extensions\/[^/]+$/, {
    timeout: options.connectTimeoutMs,
  });
  await page.getByTestId('extension-detail-name').waitFor({ state: 'visible' });
  assert.match(
    (await page.getByTestId('extension-detail-name').textContent()) ?? '',
    new RegExp(name),
  );

  await page.getByTestId('extension-version-value').fill('1.0.0');
  await page.getByTestId('extension-version-path').fill('relative/path');
  await page.getByTestId('extension-version-path-error').waitFor({ state: 'visible' });
  await page.getByTestId('extension-version-path').fill('/home/bpane/bpane-test-extension');
  await page.getByTestId('extension-version-submit').click();
  await page.getByTestId('extension-action-success').waitFor({ state: 'visible' });
  await page.getByTestId('extension-detail-version').waitFor({ state: 'visible' });
  assert.match((await page.getByTestId('extension-detail-version').textContent()) ?? '', /1\.0\.0/);

  await page.getByTestId('extension-toggle-enabled').click();
  await waitForText(page, 'extension-detail-state', 'Disabled', options.connectTimeoutMs);
  await page.getByTestId('extension-toggle-enabled').click();
  await waitForText(page, 'extension-detail-state', 'Enabled', options.connectTimeoutMs);
  await page.reload({ waitUntil: 'domcontentloaded' });
  await page.getByTestId('extension-detail-name').waitFor({ state: 'visible' });
  await assertNoBodyHorizontalOverflow(page, 'extension detail');

  const id = new URL(page.url()).pathname.split('/').at(-1);
  assert.ok(id, 'extension detail URL must contain an id');
  return { id, name };
}

async function waitForText(page, testId, expected, timeoutMs) {
  await page.getByTestId(testId).waitFor({ state: 'visible', timeout: timeoutMs });
  await page.waitForFunction(
    ({ id, text }) => document.querySelector(`[data-testid="${id}"]`)?.textContent?.includes(text),
    { id: testId, text: expected },
    { timeout: timeoutMs },
  );
}
