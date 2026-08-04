import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import type {
  BrowserSessionHandle,
  BrowserSessionStatsSnapshot,
  BrowserSessionTileRuntimeStats,
} from '$lib/session-preview/browser-session-types';
import { sessionPayload } from '$lib/test-utils/session-fixtures';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import SessionPreviewRoute from './SessionPreviewRoute.svelte';

beforeEach(() => {
  window.history.replaceState(null, '', 'http://localhost:3000/admin-new/sessions/session-1/preview');
});

afterEach(async () => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  await cleanupRenderedComponents();
});

describe('SessionPreviewRoute', () => {
  it('reports missing OIDC configuration in a standalone preview window', async () => {
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async () => new Response('', { status: 404 })));

    const target = renderComponent(SessionPreviewRoute);

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-preview-error').textContent).toContain('OIDC authentication is not configured');
    });
  });

  it('loads the selected session and connects the popup viewport', async () => {
    const disconnect = vi.fn();
    const connect = vi.fn(async () => ({
      sessionId: 'session-1',
      gatewayUrl: 'https://localhost:4433/session/session-1',
      handle: { disconnect },
    }));
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async (input) => {
      const url = String(input);
      if (url.endsWith('/api/v1/sessions/session-1')) {
        return jsonResponse(sessionPayload(), 200);
      }
      return new Response('not found', { status: 404 });
    }));

    const target = renderComponent(SessionPreviewRoute, {
      authContext: authContext(),
      connectorFactory: () => ({ connect }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-preview-status').textContent).toContain('Connected');
    });
    expect(byTestId(target, 'session-preview-title').textContent).toContain('session-1');
    expect(connect).toHaveBeenCalledOnce();

    byTestId(target, 'session-preview-disconnect').click();
    expect(disconnect).toHaveBeenCalledOnce();
    await vi.waitFor(() => {
      expect(byTestId(target, 'session-preview-status').textContent).toContain('Disconnected');
    });
  });

  it('shows connection failures without embedding a broken canvas', async () => {
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async () => jsonResponse(sessionPayload(), 200)));
    const target = renderComponent(SessionPreviewRoute, {
      authContext: authContext(),
      connectorFactory: () => ({
        connect: async () => {
          throw new Error('preview failed');
        },
      }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-preview-error').textContent).toContain('preview failed');
    });
  });

  it('opens the transition metrics drawer for the active preview connection', async () => {
    let now = 1_000;
    vi.spyOn(performance, 'now').mockImplementation(() => now);
    const metrics = createMutableMetrics();
    const disconnect = vi.fn();
    const handle = createMetricsHandle(metrics, disconnect);
    const connect = vi.fn(async () => ({
      sessionId: 'session-1',
      gatewayUrl: 'https://localhost:4433/session/session-1',
      handle,
    }));
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async (input) => {
      const url = String(input);
      if (url.endsWith('/api/v1/sessions/session-1')) {
        return jsonResponse(sessionPayload(), 200);
      }
      return new Response('not found', { status: 404 });
    }));

    const target = renderComponent(SessionPreviewRoute, {
      authContext: authContext(),
      connectorFactory: () => ({ connect }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-preview-status').textContent).toContain('Connected');
    });
    byTestId(target, 'session-preview-metrics-toggle').click();
    await vi.waitFor(() => {
      expect(byTestId(target, 'session-preview-metrics-drawer')).toBeTruthy();
    });
    byTestId(target, 'session-preview-metrics-start').click();
    await vi.waitFor(() => {
      expect(byTestId(target, 'session-preview-metrics-stop').hasAttribute('disabled')).toBe(false);
    });
    metrics.frameCount = 30;
    metrics.tileRuntime = {
      ...metrics.tileRuntime,
      hits: 40,
      misses: 10,
      hitRate: 80,
      bytes: 98_304,
      maxBatchCommands: 12,
      pendingCommandsHighWaterMark: 5,
    };
    metrics.sessionStats = {
      ...metrics.sessionStats,
      transfer: {
        ...metrics.sessionStats.transfer,
        rxBytes: 160_000,
        txBytes: 8_000,
      },
      tiles: {
        ...metrics.sessionStats.tiles,
        commandBytes: 80_000,
        imageCommands: 20,
        videoCommands: 4,
        drawCommands: 24,
        totalCommands: 48,
        cacheHitRateObserved: 80,
        scrollComposition: {
          ...metrics.sessionStats.tiles.scrollComposition,
          scrollBatches: 3,
          scrollPotentialTiles: 50,
          scrollSavedTiles: 40,
        },
      },
      video: {
        ...metrics.sessionStats.video,
        datagrams: 6,
        datagramBytes: 30_000,
      },
    };
    now = 3_000;
    byTestId(target, 'session-preview-metrics-stop').click();

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-preview-metrics-sample').textContent).toContain('15.0 fps');
    });
    expect(byTestId(target, 'session-preview-metrics-transfer').textContent).toContain('down');
    expect(byTestId(target, 'session-preview-metrics-tiles').textContent).toContain('48 commands');
    expect(byTestId(target, 'session-preview-metrics-scroll').textContent).toContain('saved 80.0%');
    expect(byTestId(target, 'session-preview-metrics-video').textContent).toContain('6 datagrams');
  });
});

function authContext(): UnifiedAdminContext {
  return {
    auth: {
      configured: true,
      authenticated: true,
      username: 'demo',
      accessToken: 'token',
      claims: null,
    },
    authConfig: null,
    accessTokenProvider: async () => 'token',
    onAuthenticationFailure: vi.fn(),
    login: async () => {},
    logout: async () => {},
  };
}

function jsonResponse(payload: unknown, status: number): Response {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

type MutableMetrics = {
  frameCount: number;
  tileRuntime: BrowserSessionTileRuntimeStats;
  sessionStats: BrowserSessionStatsSnapshot;
};

function createMetricsHandle(metrics: MutableMetrics, disconnect: () => void): BrowserSessionHandle {
  return {
    disconnect,
    getFrameCount: () => metrics.frameCount,
    getTileCacheStats: () => metrics.tileRuntime,
    getSessionStats: () => metrics.sessionStats,
    getRenderDiagnostics: () => ({
      backend: 'webgl2',
      reason: 'hardware-accelerated',
      renderer: 'ANGLE (Apple, Apple M3, Unspecified Version)',
      vendor: 'Apple',
      software: false,
    }),
  };
}

function createMutableMetrics(): MutableMetrics {
  return {
    frameCount: 0,
    tileRuntime: {
      hits: 0,
      misses: 0,
      hitRate: 0,
      size: 0,
      bytes: 0,
      evictions: 0,
      fills: 0,
      qoiDecodes: 0,
      qoiRedundant: 0,
      qoiRedundantBytes: 0,
      zstdDecodes: 0,
      zstdRedundant: 0,
      zstdRedundantBytes: 0,
      cacheMisses: 0,
      scrollCopies: 0,
      batchesQueued: 0,
      totalBatchCommands: 0,
      maxBatchCommands: 0,
      lastBatchCommands: 0,
      currentPendingCommands: 0,
      pendingCommandsHighWaterMark: 0,
    },
    sessionStats: {
      elapsedMs: 0,
      transfer: {
        rxBytes: 0,
        txBytes: 0,
        rxFrames: 0,
        txFrames: 0,
        rxByChannel: {},
        txByChannel: {},
      },
      tiles: {
        commandBytes: 0,
        imageCommands: 0,
        videoCommands: 0,
        drawCommands: 0,
        totalCommands: 0,
        cacheHitsObserved: 0,
        cacheMissesObserved: 0,
        cacheHitRateObserved: 0,
        cacheSizeObserved: 0,
        redundantQoiCommands: 0,
        redundantQoiBytes: 0,
        scrollComposition: {
          scrollBatches: 0,
          subTileScrollBatches: 0,
          scrollPotentialTiles: 0,
          scrollSavedTiles: 0,
          subTileScrollPotentialTiles: 0,
          subTileScrollSavedTiles: 0,
          subTileScrollReuseRate: 0,
          subTileComposeCandidateRate: 0,
        },
        scrollHealth: {
          hostScrollBatchesTotal: 0,
          hostScrollFallbacksTotal: 0,
          hostScrollNonQuantizedFallbacksTotal: 0,
          hostScrollResidualFullRepaintsTotal: 0,
          hostScrollResidualInteriorLimitFallbacksTotal: 0,
          hostScrollResidualLowSavedRatioFallbacksTotal: 0,
          hostScrollResidualLargeRowShiftFallbacksTotal: 0,
          hostScrollResidualOtherFallbacksTotal: 0,
          hostScrollZeroSavedBatchesTotal: 0,
          hostScrollSplitRegionBatchesTotal: 0,
          hostScrollStickyBandBatchesTotal: 0,
          hostScrollChromeTilesTotal: 0,
          hostScrollExposedStripTilesTotal: 0,
          hostScrollInteriorResidualTilesTotal: 0,
          hostScrollEdgeStripResidualTilesTotal: 0,
          hostScrollSmallEdgeStripResidualTilesTotal: 0,
          hostScrollSmallEdgeStripResidualRowsTotal: 0,
          hostScrollSmallEdgeStripResidualAreaPxTotal: 0,
          hostSentHashEntries: 0,
          hostSentHashEvictionsTotal: 0,
          hostCacheMissReportsTotal: 0,
          hostFallbackRate: 0,
          hostFallbackRateRecent20: 0,
          hostFallbackRateRecent50: 0,
          hostFallbackRateRecent20Batches: 0,
          hostFallbackRateRecent50Batches: 0,
          hostScrollPotentialTilesTotal: 0,
          hostScrollSavedTilesTotal: 0,
          hostScrollSavedRate: 0,
          lastHostScrollStatsAtMs: 0,
        },
      },
      video: {
        decodedFrames: 0,
        droppedFrames: 0,
        datagrams: 0,
        datagramBytes: 0,
      },
    },
  };
}
