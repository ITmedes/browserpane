import { describe, expect, it } from 'vitest';

import {
  PROTOCOL_CAPABILITY as capability,
  ProtocolNegotiationError,
  ProtocolNegotiator,
  ProtocolSupportError,
  type ClientHello,
  type ProtocolFailureCode,
  type ProtocolSupport,
  type ServerSelection,
} from '../protocol.js';

const negotiator = new ProtocolNegotiator();
const support: ProtocolSupport = {
  versions: [1],
  capabilities: [
    capability.h264Video,
    capability.roiVideo,
    capability.audioOpus,
    capability.clipboardText,
    capability.fileTransfer,
  ],
};

describe('ProtocolNegotiator', () => {
  it('selects the highest common version and deterministic mutual capabilities', () => {
    const hello: ClientHello = {
      versions: [1],
      requiredCapabilities: [capability.clipboardText],
      optionalCapabilities: [capability.h264Video, capability.roiVideo, 0x00FF],
    };
    const selection = negotiator.select(hello, support);
    expect(selection).toEqual({
      selectedVersion: 1,
      capabilities: [
        capability.h264Video,
        capability.roiVideo,
        capability.clipboardText,
      ],
    });
    expect(() => negotiator.validateSelection(hello, support, selection)).not.toThrow();
  });

  it('rejects unknown, unsupported, and dependency-invalid required capabilities', () => {
    const requiredCases = [0x00FF, capability.tileCache, capability.roiVideo];
    for (const required of requiredCases) {
      expectFailure(
        () => negotiator.select({
          versions: [1],
          requiredCapabilities: [required],
          optionalCapabilities: [],
        }, support),
        'required_protocol_capability_missing',
      );
    }
    expectFailure(
      () => negotiator.select({
        versions: [2],
        requiredCapabilities: [],
        optionalCapabilities: [],
      }, support),
      'unsupported_protocol_version',
    );
  });

  it('refuses a lower common version and mismatched capability reply', () => {
    const hello: ClientHello = {
      versions: [1, 2],
      requiredCapabilities: [],
      optionalCapabilities: [capability.clipboardText],
    };
    const twoVersionSupport: ProtocolSupport = {
      versions: [1, 2],
      capabilities: [capability.clipboardText],
    };
    expectFailure(
      () => negotiator.validateSelection(hello, twoVersionSupport, {
        selectedVersion: 1,
        capabilities: [capability.clipboardText],
      }),
      'protocol_downgrade_refused',
    );
    expectFailure(
      () => negotiator.validateSelection(hello, twoVersionSupport, {
        selectedVersion: 2,
        capabilities: [],
      }),
      'protocol_selection_mismatch',
    );
  });

  it('rejects ambiguous or incomplete local support profiles', () => {
    expectSupportFailure(
      { versions: [2, 1], capabilities: [] },
      'versions_not_canonical',
    );
    expectSupportFailure({
      versions: [1],
      capabilities: [capability.audioPcmS16Le, capability.audioOpus],
    }, 'multiple_desktop_audio_codecs');
    expectSupportFailure({
      versions: [1],
      capabilities: [capability.roiVideo],
    }, 'capability_dependency_missing');
  });

  it('never selects a capability outside either peer set', () => {
    for (let seed = 0; seed < 64; seed += 1) {
      const offered = [
        capability.h264Video,
        capability.roiVideo,
        capability.clipboardText,
        capability.fileTransfer,
      ].filter((_, index) => (seed & (1 << index)) !== 0);
      if (offered.includes(capability.roiVideo)
        && !offered.includes(capability.h264Video)) {
        offered.unshift(capability.h264Video);
      }
      const hello = {
        versions: [1],
        requiredCapabilities: [],
        optionalCapabilities: offered,
      };
      const selection = negotiator.select(hello, support);
      const offeredIds = new Set<number>(offered);
      const supportedIds = new Set<number>(support.capabilities);
      expect(selection.capabilities.every((id) => offeredIds.has(id))).toBe(true);
      expect(selection.capabilities.every((id) => supportedIds.has(id))).toBe(true);
    }
  });
});

function expectFailure(action: () => unknown, code: ProtocolFailureCode): void {
  try {
    action();
    throw new Error('expected protocol negotiation failure');
  } catch (error) {
    if (!(error instanceof ProtocolNegotiationError)) {
      throw error;
    }
    expect(error.code).toBe(code);
  }
}

function expectSupportFailure(
  invalidSupport: ProtocolSupport,
  code: ProtocolSupportError['code'],
): void {
  try {
    negotiator.select({
      versions: [1],
      requiredCapabilities: [],
      optionalCapabilities: [],
    }, invalidSupport);
    throw new Error('expected protocol support failure');
  } catch (error) {
    if (!(error instanceof ProtocolSupportError)) {
      throw error;
    }
    expect(error.code).toBe(code);
  }
}
