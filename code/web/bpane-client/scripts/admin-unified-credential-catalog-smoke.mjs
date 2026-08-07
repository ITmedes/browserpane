import assert from 'node:assert/strict';
import { adminRouteUrl, assertNoBodyHorizontalOverflow } from './admin-unified-smoke-lib.mjs';

export async function exerciseCredentialCatalog(page, options, runLabel, secretMarker) {
  const name = `Catalog credential ${runLabel}`;
  await page.goto(adminRouteUrl(options, 'credential-bindings/new'), {
    waitUntil: 'domcontentloaded',
  });
  await page.getByTestId('credential-binding-create-form').waitFor({ state: 'visible' });
  await page.getByTestId('credential-binding-name').fill(name);
  await page.getByTestId('credential-binding-origins').fill('not-an-origin');
  await page.getByTestId('credential-binding-origins-error').waitFor({ state: 'visible' });
  await page.getByTestId('credential-binding-origins').fill('http://web:8080');
  await page.getByTestId('credential-binding-secret-payload').fill('[]');
  await page.getByTestId('credential-binding-secret-payload-error').waitFor({ state: 'visible' });
  await page
    .getByTestId('credential-binding-secret-payload')
    .fill(JSON.stringify({ username: 'catalog-user', password: secretMarker }));
  await page
    .getByTestId('credential-binding-labels')
    .fill(`suite=admin-resource-catalogs\nrun=${runLabel}`);
  await page.getByTestId('credential-binding-create-submit').click();
  await page.waitForURL(/\/admin-new\/credential-bindings\/[^/]+$/, {
    timeout: options.connectTimeoutMs,
  });
  await page.getByTestId('credential-binding-detail-name').waitFor({ state: 'visible' });
  assert.match(
    (await page.getByTestId('credential-binding-detail-name').textContent()) ?? '',
    new RegExp(name),
  );
  await assertDocumentExcludes(page, secretMarker);
  await page.reload({ waitUntil: 'domcontentloaded' });
  await page.getByTestId('credential-binding-write-only').waitFor({ state: 'visible' });
  await assertDocumentExcludes(page, secretMarker);
  await assertNoBodyHorizontalOverflow(page, 'credential binding detail');

  const id = new URL(page.url()).pathname.split('/').at(-1);
  assert.ok(id, 'credential binding detail URL must contain an id');
  return { id, name, retainedFixture: true };
}

async function assertDocumentExcludes(page, secret) {
  const documentHtml = await page.content();
  assert.equal(documentHtml.includes(secret), false, 'credential secret leaked into rendered HTML');
  assert.equal(
    (await page.locator('body').innerText()).includes(secret),
    false,
    'credential secret leaked into visible UI',
  );
}
