import assert from 'node:assert/strict';

const CONTROL_CHANNEL = 0x0A;

export class GatewayProtocolSmokeWire {
  static hello(versions, requiredCapabilities = [], optionalCapabilities = []) {
    return this.frame(CONTROL_CHANNEL, [
      0x0A,
      ...this.list(versions),
      ...this.list(requiredCapabilities),
      ...this.list(optionalCapabilities),
    ]);
  }

  static malformedHello() {
    return this.frame(CONTROL_CHANNEL, [0x0A, 0x01, 0x01, 0x00, 0x00]);
  }

  static resolutionRequest(width = 1280, height = 720) {
    return this.frame(CONTROL_CHANNEL, [
      0x01,
      ...this.u16(width),
      ...this.u16(height),
    ]);
  }

  static prematureServerSelection() {
    return this.frame(CONTROL_CHANNEL, [0x0B, 0x01, 0x00, 0x00]);
  }

  static oversizedFrameHeader() {
    return [CONTROL_CHANNEL, 0x01, 0x00, 0x00, 0x01];
  }

  static assertSelection(frames, expectedCapabilities) {
    const frame = this.controlFrame(frames, 0x0B);
    assert.equal(this.readU16(frame.payload, 1), 1, 'gateway selected protocol version 1');
    const count = frame.payload[3];
    assert.equal(frame.payload.length, 4 + count * 2, 'selection has a canonical payload length');
    const capabilities = [];
    for (let offset = 4; offset < frame.payload.length; offset += 2) {
      capabilities.push(this.readU16(frame.payload, offset));
    }
    assert.deepEqual(capabilities, expectedCapabilities, 'gateway selected the offered subset');
  }

  static assertSessionReady(frames, allowedFlags) {
    const selectionIndex = frames.findIndex((candidate) => (
      candidate.channel === CONTROL_CHANNEL && candidate.payload[0] === 0x0B
    ));
    const readyIndex = frames.findIndex((candidate) => (
      candidate.channel === CONTROL_CHANNEL && candidate.payload[0] === 0x03
    ));
    assert.ok(selectionIndex >= 0 && selectionIndex < readyIndex, 'selection precedes SessionReady');
    const frame = this.controlFrame(frames, 0x03);
    assert.equal(frame.payload.length, 3, 'SessionReady has the protocol-v1 shape');
    assert.equal(frame.payload[1], 1, 'SessionReady reports the selected version');
    assert.equal(frame.payload[2] & ~allowedFlags, 0, 'SessionReady stays within selected capabilities');
  }

  static assertReject(frames, expectedFailure) {
    const frame = this.controlFrame(frames, 0x0C);
    assert.equal(frame.payload.length, 3, 'ProtocolReject has the bounded typed shape');
    assert.equal(this.readU16(frame.payload, 1), expectedFailure, 'typed rejection reason');
  }

  static frame(channel, payload) {
    const length = payload.length;
    return [
      channel,
      length & 0xFF,
      (length >>> 8) & 0xFF,
      (length >>> 16) & 0xFF,
      (length >>> 24) & 0xFF,
      ...payload,
    ];
  }

  static list(values) {
    assert.ok(values.length <= 255, 'wire list count is bounded');
    return [values.length, ...values.flatMap((value) => this.u16(value))];
  }

  static u16(value) {
    return [value & 0xFF, (value >>> 8) & 0xFF];
  }

  static readU16(bytes, offset) {
    return bytes[offset] | (bytes[offset + 1] << 8);
  }

  static controlFrame(frames, tag) {
    const frame = frames.find((candidate) => (
      candidate.channel === CONTROL_CHANNEL && candidate.payload[0] === tag
    ));
    assert.ok(frame, `expected control frame tag 0x${tag.toString(16)}`);
    return frame;
  }
}
