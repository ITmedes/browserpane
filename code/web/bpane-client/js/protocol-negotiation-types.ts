/** Protocol-v1 negotiation identifiers and public data models. */

export const PROTOCOL_CAPABILITY = {
  tileZstd: 0x0001,
  tileCache: 0x0002,
  tileScroll: 0x0003,
  h264Video: 0x0004,
  roiVideo: 0x0005,
  audioPcmS16Le: 0x0006,
  audioAdpcmImaStereo: 0x0007,
  audioOpus: 0x0008,
  microphoneOpus: 0x0009,
  cameraH264AnnexB: 0x000A,
  clipboardText: 0x000B,
  fileTransfer: 0x000C,
  extendedKeyboard: 0x000D,
  clientAccessState: 0x000E,
} as const;

export type ProtocolCapability =
  (typeof PROTOCOL_CAPABILITY)[keyof typeof PROTOCOL_CAPABILITY];

export const PROTOCOL_FAILURE_ID = {
  unsupported_protocol_version: 0x0001,
  required_protocol_capability_missing: 0x0002,
  malformed_protocol_hello: 0x0003,
  protocol_downgrade_refused: 0x0004,
  protocol_handshake_timeout: 0x0005,
  protocol_selection_mismatch: 0x0006,
  unexpected_protocol_frame: 0x0007,
  protocol_frame_too_large: 0x0008,
  protocol_pending_buffer_limit: 0x0009,
} as const;

export type ProtocolFailureCode = keyof typeof PROTOCOL_FAILURE_ID;

export type ClientHello = {
  readonly versions: readonly number[];
  readonly requiredCapabilities: readonly number[];
  readonly optionalCapabilities: readonly number[];
};

export type ServerSelection = {
  readonly selectedVersion: number;
  readonly capabilities: readonly number[];
};

export type ProtocolSupport = {
  readonly versions: readonly number[];
  readonly capabilities: readonly ProtocolCapability[];
};

export type NegotiationMessage =
  | { readonly type: 'client_hello'; readonly hello: ClientHello }
  | { readonly type: 'server_selection'; readonly selection: ServerSelection }
  | { readonly type: 'protocol_reject'; readonly failure: ProtocolFailureCode };

export type ProtocolSupportErrorCode =
  | 'versions_not_canonical'
  | 'capabilities_not_canonical'
  | 'capability_dependency_missing'
  | 'multiple_desktop_audio_codecs';

/** A stable protocol failure that never includes peer payload bytes. */
export class ProtocolNegotiationError extends Error {
  public readonly code: ProtocolFailureCode;

  public constructor(code: ProtocolFailureCode) {
    super(code);
    this.name = 'ProtocolNegotiationError';
    this.code = code;
  }
}

/** Invalid local support configuration, distinct from a peer rejection. */
export class ProtocolSupportError extends Error {
  public readonly code: ProtocolSupportErrorCode;

  public constructor(code: ProtocolSupportErrorCode) {
    super(code);
    this.name = 'ProtocolSupportError';
    this.code = code;
  }
}

/** Lookups for the frozen v1 capability and failure registries. */
export class ProtocolNegotiationRegistry {
  private static readonly capabilities = new Set<number>(
    Object.values(PROTOCOL_CAPABILITY),
  );

  private static readonly failuresById = new Map<number, ProtocolFailureCode>(
    [
      [PROTOCOL_FAILURE_ID.unsupported_protocol_version, 'unsupported_protocol_version'],
      [
        PROTOCOL_FAILURE_ID.required_protocol_capability_missing,
        'required_protocol_capability_missing',
      ],
      [PROTOCOL_FAILURE_ID.malformed_protocol_hello, 'malformed_protocol_hello'],
      [PROTOCOL_FAILURE_ID.protocol_downgrade_refused, 'protocol_downgrade_refused'],
      [PROTOCOL_FAILURE_ID.protocol_handshake_timeout, 'protocol_handshake_timeout'],
      [PROTOCOL_FAILURE_ID.protocol_selection_mismatch, 'protocol_selection_mismatch'],
      [PROTOCOL_FAILURE_ID.unexpected_protocol_frame, 'unexpected_protocol_frame'],
      [PROTOCOL_FAILURE_ID.protocol_frame_too_large, 'protocol_frame_too_large'],
      [PROTOCOL_FAILURE_ID.protocol_pending_buffer_limit, 'protocol_pending_buffer_limit'],
    ],
  );

  public static isKnownCapability(value: number): value is ProtocolCapability {
    return ProtocolNegotiationRegistry.capabilities.has(value);
  }

  public static failureId(code: ProtocolFailureCode): number {
    return PROTOCOL_FAILURE_ID[code];
  }

  public static isFailureCode(value: unknown): value is ProtocolFailureCode {
    return typeof value === 'string'
      && Object.hasOwn(PROTOCOL_FAILURE_ID, value);
  }

  public static failureCode(id: number): ProtocolFailureCode | undefined {
    return ProtocolNegotiationRegistry.failuresById.get(id);
  }

  public static dependency(capability: ProtocolCapability): ProtocolCapability | undefined {
    return capability === PROTOCOL_CAPABILITY.roiVideo
      ? PROTOCOL_CAPABILITY.h264Video
      : undefined;
  }

  public static isDesktopAudio(capability: ProtocolCapability): boolean {
    return capability === PROTOCOL_CAPABILITY.audioPcmS16Le
      || capability === PROTOCOL_CAPABILITY.audioAdpcmImaStereo
      || capability === PROTOCOL_CAPABILITY.audioOpus;
  }
}
