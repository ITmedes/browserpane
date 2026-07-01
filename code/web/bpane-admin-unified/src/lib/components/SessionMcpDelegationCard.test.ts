import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import type { McpDelegationViewModel } from '$lib/mcp/mcp-delegation-view-model';
import SessionMcpDelegationCard from './SessionMcpDelegationCard.svelte';

afterEach(cleanupRenderedComponents);

describe('SessionMcpDelegationCard', () => {
  it('renders endpoint metadata and dispatches MCP actions', () => {
    const onAuthorize = vi.fn();
    const onSetDefault = vi.fn();
    const onCopyEndpoint = vi.fn();
    const target = renderComponent(SessionMcpDelegationCard, {
      viewModel: model({
        statusLabel: 'Not authorized',
        endpointUrl: 'http://localhost:8931/sessions/session-1/mcp',
        canAuthorize: true,
        canSetDefault: true,
        canCopyEndpoint: true,
      }),
      onAuthorize,
      onSetDefault,
      onCopyEndpoint,
    });

    expect(byTestId(target, 'mcp-delegation-status').textContent).toContain('Not authorized');
    expect(byTestId(target, 'mcp-endpoint-url').textContent).toContain('/sessions/session-1/mcp');
    expect(
      byTestId(target, 'mcp-endpoint-row').compareDocumentPosition(byTestId(target, 'mcp-summary-row'))
      & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(
      byTestId(target, 'mcp-endpoint-row').compareDocumentPosition(byTestId(target, 'mcp-authorize'))
      & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();

    byTestId(target, 'mcp-authorize').click();
    byTestId(target, 'mcp-set-default').click();
    byTestId(target, 'mcp-copy-endpoint').click();

    expect(onAuthorize).toHaveBeenCalledOnce();
    expect(onSetDefault).toHaveBeenCalledOnce();
    expect(onCopyEndpoint).toHaveBeenCalledOnce();
  });

  it('shows action feedback and disables unavailable controls', () => {
    const target = renderComponent(SessionMcpDelegationCard, {
      viewModel: model({
        statusLabel: 'Authorized default',
        statusTone: 'success',
        canClearDefault: true,
      }),
      actionState: { status: 'success', message: 'This session is now the default MCP session.' },
    });

    expect((byTestId(target, 'mcp-authorize') as HTMLButtonElement).disabled).toBe(true);
    expect((byTestId(target, 'mcp-clear-default') as HTMLButtonElement).disabled).toBe(false);
    expect(byTestId(target, 'mcp-action-success').textContent).toContain('default MCP session');
  });
});

function model(overrides: Partial<McpDelegationViewModel> = {}): McpDelegationViewModel {
  return {
    title: 'Local MCP bridge',
    statusLabel: 'Not authorized',
    statusTone: 'neutral',
    description: 'session-1 is not authorized for Local MCP bridge.',
    endpointUrl: null,
    defaultSessionLabel: 'No default session selected',
    clientSummary: 'No MCP clients are attached to this session endpoint.',
    ownershipLabel: 'MCP ownership inactive',
    bridgeSummary: 'ok',
    canRefresh: true,
    canAuthorize: false,
    canRevoke: false,
    canSetDefault: false,
    canClearDefault: false,
    canCopyEndpoint: false,
    ...overrides,
  };
}
