export default async function run({ page, input, artifacts }) {
  const holdMs = resolveHoldMs(input.hold_ms);
  await page.goto(input.target_url, {
    waitUntil: 'domcontentloaded',
    timeout: 60_000,
  });
  if (holdMs > 0) await page.waitForTimeout(holdMs);
  const body = (await page.locator('body').innerText()).trim();
  const produced = await artifacts.uploadTextFile({
    workspaceId: input.output_workspace_id,
    fileName: 'single-node-qualification.txt',
    mediaType: 'text/plain; charset=utf-8',
    provenance: {
      origin: 'single-node-qualification',
      kind: 'produced_file',
    },
    text: `body=${body}\nurl=${page.url()}\n`,
  });
  return {
    body,
    final_url: page.url(),
    output_file_id: produced.file_id,
  };
}

function resolveHoldMs(value) {
  if (value === undefined) return 0;
  if (!Number.isSafeInteger(value) || value < 0 || value > 30_000) {
    throw new Error('hold_ms must be an integer between 0 and 30000');
  }
  return value;
}
