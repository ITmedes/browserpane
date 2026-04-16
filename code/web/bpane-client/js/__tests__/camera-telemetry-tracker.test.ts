import { describe, expect, it } from 'vitest';

import { CameraTelemetryTracker } from '../camera/camera-telemetry-tracker.js';

describe('CameraTelemetryTracker', () => {
  it('records capture counts, encoded chunk totals, and average encode time', () => {
    const tracker = new CameraTelemetryTracker();

    tracker.recordCapture();
    tracker.recordEncodedChunk({
      chunkType: 'key',
      chunkByteLength: 512,
      sendResult: 'queued',
      encodeTimeMs: 7,
    });
    tracker.recordEncodedChunk({
      chunkType: 'delta',
      chunkByteLength: 256,
      sendResult: 'sent',
      encodeTimeMs: 5,
    });

    expect(tracker.getMetrics()).toEqual({
      framesCaptured: 1,
      framesEncoded: 2,
      keyframesEncoded: 1,
      encodedBytes: 768,
      transportFramesQueued: 1,
      transportFramesReplaced: 0,
      encoderQueueDrops: 0,
      averageEncodeTimeMs: 6,
      maxEncodeTimeMs: 7,
      profileUpgrades: 0,
      profileDowngrades: 0,
      reconfigurations: 0,
    });
    expect(tracker.getWindowStats()).toEqual({
      transportQueued: 1,
      transportReplaced: 0,
      encoderQueueDrops: 0,
      encodeTimeMs: 12,
      encodedSamples: 2,
    });
  });

  it('tracks transport replacement and encoder queue drops in the adaptation window', () => {
    const tracker = new CameraTelemetryTracker();

    tracker.recordEncoderQueueDrop();
    tracker.recordEncodedChunk({
      chunkType: 'delta',
      chunkByteLength: 64,
      sendResult: 'replaced',
    });

    expect(tracker.getMetrics()).toMatchObject({
      transportFramesReplaced: 1,
      encoderQueueDrops: 1,
    });
    expect(tracker.getWindowStats()).toEqual({
      transportQueued: 0,
      transportReplaced: 1,
      encoderQueueDrops: 1,
      encodeTimeMs: 0,
      encodedSamples: 0,
    });
  });

  it('counts upgrades and downgrades only for real profile transitions', () => {
    const tracker = new CameraTelemetryTracker();

    tracker.recordProfileChange(-1, 0);
    tracker.recordProfileChange(0, 1);
    tracker.recordProfileChange(1, 0);
    tracker.recordProfileChange(0, 0);

    expect(tracker.getMetrics()).toMatchObject({
      profileUpgrades: 1,
      profileDowngrades: 1,
      reconfigurations: 2,
    });
  });

  it('resets the adaptation window without touching accumulated totals', () => {
    const tracker = new CameraTelemetryTracker();

    tracker.recordCapture();
    tracker.recordEncoderQueueDrop();
    tracker.recordEncodedChunk({
      chunkType: 'delta',
      chunkByteLength: 42,
      sendResult: 'queued',
      encodeTimeMs: 3,
    });

    tracker.resetWindow();

    expect(tracker.getWindowStats()).toEqual({
      transportQueued: 0,
      transportReplaced: 0,
      encoderQueueDrops: 0,
      encodeTimeMs: 0,
      encodedSamples: 0,
    });
    expect(tracker.getMetrics()).toMatchObject({
      framesCaptured: 1,
      framesEncoded: 1,
      transportFramesQueued: 1,
      encoderQueueDrops: 1,
    });
  });

  it('fully resets totals and window state', () => {
    const tracker = new CameraTelemetryTracker();

    tracker.recordCapture();
    tracker.recordProfileChange(0, 1);
    tracker.recordEncoderQueueDrop();
    tracker.recordEncodedChunk({
      chunkType: 'key',
      chunkByteLength: 100,
      sendResult: 'replaced',
      encodeTimeMs: 9,
    });

    tracker.reset();

    expect(tracker.getMetrics()).toEqual({
      framesCaptured: 0,
      framesEncoded: 0,
      keyframesEncoded: 0,
      encodedBytes: 0,
      transportFramesQueued: 0,
      transportFramesReplaced: 0,
      encoderQueueDrops: 0,
      averageEncodeTimeMs: 0,
      maxEncodeTimeMs: 0,
      profileUpgrades: 0,
      profileDowngrades: 0,
      reconfigurations: 0,
    });
    expect(tracker.getWindowStats()).toEqual({
      transportQueued: 0,
      transportReplaced: 0,
      encoderQueueDrops: 0,
      encodeTimeMs: 0,
      encodedSamples: 0,
    });
  });
});
