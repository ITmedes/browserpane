export default async function run({ page, input, artifacts }) {
  await page.goto(input.target_url, {
    waitUntil: 'domcontentloaded',
    timeout: 60_000,
  });
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
