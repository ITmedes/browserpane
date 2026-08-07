import { describe, expect, it, vi } from 'vitest';
import { waitForMcpControlSession } from './mcp-bridge-health-poller';

describe('waitForMcpControlSession', () => {
  it('tolerates transient health failures until the expected default is visible', async () => {
    const loadHealth = vi
      .fn()
      .mockRejectedValueOnce(new Error('HTTP 503'))
      .mockResolvedValueOnce(health(null))
      .mockResolvedValueOnce(health('session-1'));

    await expect(
      waitForMcpControlSession(loadHealth, 'session-1', { timeoutMs: 100, intervalMs: 1 }),
    ).resolves.toMatchObject({ control_session_id: 'session-1' });
    expect(loadHealth).toHaveBeenCalledTimes(3);
  });

  it('reports the last observed mismatch when convergence times out', async () => {
    await expect(
      waitForMcpControlSession(async () => health('other'), 'session-1', {
        timeoutMs: 2,
        intervalMs: 1,
      }),
    ).rejects.toThrow('MCP bridge default is other, expected session-1');
  });
});

function health(controlSessionId: string | null) {
  return {
    status: 'ok',
    clients: 0,
    control_session_id: controlSessionId,
    control_session_state: null,
    control_session_backend_delegated: false,
    bridge_alignment: null,
    managed_sessions: [],
  };
}
