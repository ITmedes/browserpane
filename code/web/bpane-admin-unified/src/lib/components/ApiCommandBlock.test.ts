import { afterEach, describe, expect, it, vi } from 'vitest';
import { byTestId, cleanupRenderedComponents, renderComponent } from '$lib/test-utils/svelte-component-test';
import ApiCommandBlock from './ApiCommandBlock.svelte';

afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});

describe('ApiCommandBlock', () => {
  it('copies the exact command and renders success feedback', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal('navigator', { clipboard: { writeText } });
    const target = renderComponent(ApiCommandBlock, {
      command: 'curl "${BPANE_BASE_URL}/api/v1/projects"',
      label: 'listProjects',
      testId: 'command-test',
    });
    byTestId(target, 'command-test-copy').click();
    await vi.waitFor(() => expect(byTestId(target, 'command-test-feedback').textContent).toContain('copied'));
    expect(writeText).toHaveBeenCalledWith('curl "${BPANE_BASE_URL}/api/v1/projects"');
  });

  it('keeps the command visible when clipboard access fails', async () => {
    vi.stubGlobal('navigator', { clipboard: { writeText: vi.fn().mockRejectedValue(new Error('denied')) } });
    const target = renderComponent(ApiCommandBlock, {
      command: 'curl example',
      label: 'example',
      testId: 'command-failure',
    });
    byTestId(target, 'command-failure-copy').click();
    await vi.waitFor(() => expect(byTestId(target, 'command-failure-feedback').textContent).toContain('failed'));
    expect(byTestId(target, 'command-failure').textContent).toContain('curl');
  });
});
