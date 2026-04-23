/**
 * BrowserPane Client - Public TypeScript API
 *
 * Public facade only. The concrete session implementation lives in
 * `session/bpane-session.ts`.
 */

export type {
  BpaneOptions,
  RenderBackendPreference,
  SessionRecordingOptions,
  SessionCapabilities,
} from './bpane-types.js';
export { BpaneSession } from './session/bpane-session.js';

// Re-export stats types for backward compatibility
export type { ChannelTransferStats, TileCommandStats } from './session-stats.js';
export type { SessionStatsSnapshot as SessionStats } from './session-stats.js';
export type { WebGLRendererDiagnostics as RenderDiagnostics } from './webgl-compositor.js';

// Re-export layout helpers from input-controller.
export { inferLayoutName, inferLayoutHint, InputController } from './input-controller.js';
export type { InputControllerDeps } from './input-controller.js';

// Re-export utilities for external use.
export { fnvHash } from './hash.js';
export { domCodeToEvdev, buildModifiers, normalizeScroll, createScrollState } from './input-map.js';
export { NalReassembler, parseTileInfo, getNalType } from './nal.js';
export { encodeFrame, parseFrames } from './protocol.js';
export type { TileInfo, ReassembledNal } from './nal.js';
export type { ScrollState } from './input-map.js';
export type { ParsedFrame } from './protocol.js';
export { TileCompositor } from './tile-compositor.js';
export { TileCache, parseTileMessage, CH_TILES } from './tile-cache.js';
export type { TileCommand, TileGridConfig } from './tile-cache.js';
