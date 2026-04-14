import { beforeEach, describe, expect, it, vi } from 'vitest';
import { InputController } from '../input-controller.js';
import { CH_CLIPBOARD, CH_INPUT, encodeFrame } from '../protocol.js';
import { wireFixture } from './wire-fixtures.js';

interface SentFrame {
  channelId: number;
  payload: Uint8Array;
}

interface DecodedKeyFrame {
  keycode: number;
  down: boolean;
  modifiers: number;
  keyChar: number;
}

function decodeKeyFrames(frames: SentFrame[]): DecodedKeyFrame[] {
  return frames.map(({ payload }) => ({
    keycode: payload[1] | (payload[2] << 8) | (payload[3] << 16) | (payload[4] << 24),
    down: payload[5] === 1,
    modifiers: payload[6],
    keyChar: payload[7] | (payload[8] << 8) | (payload[9] << 16) | (payload[10] << 24),
  }));
}

function decodeClipboardText(frame: SentFrame): string {
  const length = frame.payload[1]
    | (frame.payload[2] << 8)
    | (frame.payload[3] << 16)
    | (frame.payload[4] << 24);
  return new TextDecoder().decode(frame.payload.slice(5, 5 + length));
}

function setPlatform(platform: string, userAgentDataPlatform: string): void {
  Object.defineProperty(navigator, 'platform', {
    value: platform,
    configurable: true,
  });
  Object.defineProperty(navigator, 'userAgentData', {
    value: { platform: userAgentDataPlatform },
    configurable: true,
  });
}

function dispatchKey(
  target: HTMLElement,
  type: 'keydown' | 'keyup',
  init: KeyboardEventInit & { code: string; key: string },
): void {
  target.dispatchEvent(new KeyboardEvent(type, {
    bubbles: true,
    cancelable: true,
    ...init,
  }));
}

function dispatchCompositionEnd(target: HTMLElement, data: string): void {
  const event = new CompositionEvent('compositionend', {
    bubbles: true,
    cancelable: false,
    data,
  });
  target.dispatchEvent(event);
}

function dispatchInput(target: HTMLTextAreaElement, data: string): void {
  target.value = data;
  target.dispatchEvent(new InputEvent('input', {
    bubbles: true,
    cancelable: false,
    data,
  }));
}

function createController(options: { clipboardEnabled?: boolean } = {}) {
  const canvas = document.createElement('canvas');
  canvas.tabIndex = 0;
  document.body.appendChild(canvas);

  const sentFrames: SentFrame[] = [];
  const controller = new InputController({
    canvas,
    sendFrame: (channelId, payload) => sentFrames.push({
      channelId,
      payload: new Uint8Array(payload),
    }),
    drawCursor: () => {},
    getRemoteDims: () => ({ width: 800, height: 600 }),
    clipboardEnabled: options.clipboardEnabled ?? false,
  });
  controller.serverSupportsKeyEventEx = true;
  controller.setup();
  const keyboardTarget = document.querySelector<HTMLTextAreaElement>('textarea[data-bpane-keyboard-sink="true"]');
  if (!keyboardTarget) {
    throw new Error('keyboard sink not created');
  }

  return { canvas, controller, keyboardTarget, sentFrames };
}

beforeEach(() => {
  document.body.innerHTML = '';
  setPlatform('MacIntel', 'macOS');
  Object.defineProperty(navigator, 'clipboard', {
    value: {
      readText: vi.fn().mockResolvedValue(''),
      writeText: vi.fn().mockResolvedValue(undefined),
    },
    configurable: true,
  });
  vi.useRealTimers();
});

describe('InputController locked window shortcuts', () => {
  beforeEach(() => {
    setPlatform('Linux x86_64', 'Linux');
  });

  it.each([
    { code: 'KeyW', key: 'w' },
    { code: 'KeyQ', key: 'q' },
  ])('suppresses $code when Control is held so Chromium stays open', ({ code, key }) => {
    const { keyboardTarget, controller, sentFrames } = createController();

    dispatchKey(keyboardTarget, 'keydown', { code: 'ControlLeft', key: 'Control', ctrlKey: true });
    dispatchKey(keyboardTarget, 'keydown', { code, key, ctrlKey: true });
    dispatchKey(keyboardTarget, 'keyup', { code, key, ctrlKey: true });
    dispatchKey(keyboardTarget, 'keyup', { code: 'ControlLeft', key: 'Control' });

    controller.destroy();

    const keyFrames = decodeKeyFrames(sentFrames);
    expect(keyFrames.map(({ keycode, down }) => ({ keycode, down }))).toEqual([
      { keycode: 29, down: true },
      { keycode: 29, down: false },
    ]);
  });

  it('suppresses Alt+F4 so Openbox cannot close Chromium', () => {
    const { keyboardTarget, controller, sentFrames } = createController();

    dispatchKey(keyboardTarget, 'keydown', { code: 'AltLeft', key: 'Alt', altKey: true });
    dispatchKey(keyboardTarget, 'keydown', { code: 'F4', key: 'F4', altKey: true });
    dispatchKey(keyboardTarget, 'keyup', { code: 'F4', key: 'F4', altKey: true });
    dispatchKey(keyboardTarget, 'keyup', { code: 'AltLeft', key: 'Alt' });

    controller.destroy();

    const keyFrames = decodeKeyFrames(sentFrames);
    expect(keyFrames.map(({ keycode, down }) => ({ keycode, down }))).toEqual([
      { keycode: 56, down: true },
      { keycode: 56, down: false },
    ]);
  });

  it('suppresses F11 so Chromium cannot toggle fullscreen', () => {
    const { keyboardTarget, controller, sentFrames } = createController();

    dispatchKey(keyboardTarget, 'keydown', { code: 'F11', key: 'F11' });
    dispatchKey(keyboardTarget, 'keyup', { code: 'F11', key: 'F11' });

    controller.destroy();

    expect(sentFrames).toHaveLength(0);
  });
});

describe('InputController macOS command remapping', () => {
  it('maps Cmd+C to Control+C without sending Meta to the host', () => {
    const { keyboardTarget, controller, sentFrames } = createController();

    dispatchKey(keyboardTarget, 'keydown', { code: 'MetaLeft', key: 'Meta', metaKey: true });
    dispatchKey(keyboardTarget, 'keydown', { code: 'KeyC', key: 'c', metaKey: true });
    dispatchKey(keyboardTarget, 'keyup', { code: 'KeyC', key: 'c', metaKey: true });
    dispatchKey(keyboardTarget, 'keyup', { code: 'MetaLeft', key: 'Meta' });

    controller.destroy();

    const keyFrames = decodeKeyFrames(sentFrames);
    expect(keyFrames.map(({ keycode, down }) => ({ keycode, down }))).toEqual([
      { keycode: 29, down: true },
      { keycode: 46, down: true },
      { keycode: 46, down: false },
      { keycode: 29, down: false },
    ]);
    expect(keyFrames.some(({ keycode }) => keycode === 125 || keycode === 126)).toBe(false);
  });

  it('dispatches Cmd+V atomically even if Meta is released first', () => {
    const { keyboardTarget, controller, sentFrames } = createController();

    dispatchKey(keyboardTarget, 'keydown', { code: 'MetaLeft', key: 'Meta', metaKey: true });
    dispatchKey(keyboardTarget, 'keydown', { code: 'KeyV', key: 'v', metaKey: true });
    dispatchKey(keyboardTarget, 'keyup', { code: 'MetaLeft', key: 'Meta' });
    dispatchKey(keyboardTarget, 'keyup', { code: 'KeyV', key: 'v' });

    controller.destroy();

    const keyFrames = decodeKeyFrames(sentFrames);
    expect(keyFrames.map(({ keycode, down, modifiers }) => ({
      keycode,
      down,
      modifiers,
    }))).toEqual([
      { keycode: 29, down: true, modifiers: 0 },
      { keycode: 47, down: true, modifiers: 1 },
      { keycode: 47, down: false, modifiers: 1 },
      { keycode: 29, down: false, modifiers: 0 },
    ]);
  });

  it('waits for clipboard sync before dispatching Cmd+V', async () => {
    let resolveClipboard = (_text: string) => {};
    const readText = vi.fn(() => new Promise<string>((resolve) => {
      resolveClipboard = resolve;
    }));
    Object.defineProperty(navigator, 'clipboard', {
      value: {
        readText,
        writeText: vi.fn().mockResolvedValue(undefined),
      },
      configurable: true,
    });

    const { keyboardTarget, controller, sentFrames } = createController({ clipboardEnabled: true });

    dispatchKey(keyboardTarget, 'keydown', { code: 'MetaLeft', key: 'Meta', metaKey: true });
    dispatchKey(keyboardTarget, 'keydown', { code: 'KeyV', key: 'v', metaKey: true });
    dispatchKey(keyboardTarget, 'keyup', { code: 'MetaLeft', key: 'Meta' });
    dispatchKey(keyboardTarget, 'keyup', { code: 'KeyV', key: 'v' });

    expect(sentFrames).toHaveLength(0);

    resolveClipboard('fresh paste');
    await Promise.resolve();
    await Promise.resolve();

    controller.destroy();

    expect(readText).toHaveBeenCalledTimes(1);
    expect(sentFrames.map(({ channelId }) => channelId)).toEqual([
      CH_CLIPBOARD,
      CH_INPUT,
      CH_INPUT,
      CH_INPUT,
      CH_INPUT,
    ]);
    expect(decodeClipboardText(sentFrames[0])).toBe('fresh paste');

    const keyFrames = decodeKeyFrames(sentFrames.slice(1));
    expect(keyFrames.map(({ keycode, down, modifiers }) => ({
      keycode,
      down,
      modifiers,
    }))).toEqual([
      { keycode: 29, down: true, modifiers: 0 },
      { keycode: 47, down: true, modifiers: 1 },
      { keycode: 47, down: false, modifiers: 1 },
      { keycode: 29, down: false, modifiers: 0 },
    ]);
  });

  it('ignores repeated Cmd+V keydowns while the shortcut is held', () => {
    const { keyboardTarget, controller, sentFrames } = createController();

    dispatchKey(keyboardTarget, 'keydown', { code: 'MetaLeft', key: 'Meta', metaKey: true });
    dispatchKey(keyboardTarget, 'keydown', { code: 'KeyV', key: 'v', metaKey: true });
    dispatchKey(keyboardTarget, 'keydown', {
      code: 'KeyV',
      key: 'v',
      metaKey: true,
      repeat: true,
    });
    dispatchKey(keyboardTarget, 'keyup', { code: 'MetaLeft', key: 'Meta' });
    dispatchKey(keyboardTarget, 'keyup', { code: 'KeyV', key: 'v' });

    controller.destroy();

    const keyFrames = decodeKeyFrames(sentFrames);
    expect(keyFrames.map(({ keycode, down }) => ({ keycode, down }))).toEqual([
      { keycode: 29, down: true },
      { keycode: 47, down: true },
      { keycode: 47, down: false },
      { keycode: 29, down: false },
    ]);
  });

  it('does not leave stale KeyV suppression behind if the browser drops shortcut keyup', async () => {
    vi.useFakeTimers();
    const { keyboardTarget, controller, sentFrames } = createController();

    dispatchKey(keyboardTarget, 'keydown', { code: 'MetaLeft', key: 'Meta', metaKey: true });
    dispatchKey(keyboardTarget, 'keydown', { code: 'KeyV', key: 'v', metaKey: true });
    dispatchKey(keyboardTarget, 'keyup', { code: 'MetaLeft', key: 'Meta' });
    await vi.advanceTimersByTimeAsync(800);

    dispatchKey(keyboardTarget, 'keydown', { code: 'KeyV', key: 'v' });
    dispatchKey(keyboardTarget, 'keyup', { code: 'KeyV', key: 'v' });

    controller.destroy();

    const keyFrames = decodeKeyFrames(sentFrames);
    expect(keyFrames.map(({ keycode, down, modifiers }) => ({
      keycode,
      down,
      modifiers,
    }))).toEqual([
      { keycode: 29, down: true, modifiers: 0 },
      { keycode: 47, down: true, modifiers: 1 },
      { keycode: 47, down: false, modifiers: 1 },
      { keycode: 29, down: false, modifiers: 0 },
      { keycode: 47, down: true, modifiers: 0 },
      { keycode: 47, down: false, modifiers: 0 },
    ]);
  });

  it('maps Cmd+Shift+ArrowLeft to Shift+Home without Control or Meta leakage', () => {
    const { keyboardTarget, controller, sentFrames } = createController();

    dispatchKey(keyboardTarget, 'keydown', { code: 'MetaLeft', key: 'Meta', metaKey: true });
    dispatchKey(keyboardTarget, 'keydown', {
      code: 'ShiftLeft', key: 'Shift', shiftKey: true, metaKey: true,
    });
    dispatchKey(keyboardTarget, 'keydown', {
      code: 'ArrowLeft', key: 'ArrowLeft', shiftKey: true, metaKey: true,
    });
    dispatchKey(keyboardTarget, 'keyup', {
      code: 'ArrowLeft', key: 'ArrowLeft', shiftKey: true, metaKey: true,
    });
    dispatchKey(keyboardTarget, 'keyup', {
      code: 'ShiftLeft', key: 'Shift', metaKey: true,
    });
    dispatchKey(keyboardTarget, 'keyup', { code: 'MetaLeft', key: 'Meta' });

    controller.destroy();

    const keyFrames = decodeKeyFrames(sentFrames);
    expect(keyFrames.map(({ keycode, down }) => ({ keycode, down }))).toEqual([
      { keycode: 42, down: true },
      { keycode: 102, down: true },
      { keycode: 102, down: false },
      { keycode: 42, down: false },
    ]);
    expect(keyFrames[1]?.modifiers).toBe(0x04);
    expect(keyFrames[2]?.modifiers).toBe(0x04);
    expect(keyFrames.some(({ keycode }) => [29, 97, 125, 126].includes(keycode))).toBe(false);
  });

  it('ignores repeated remap keydowns while Cmd+Shift+ArrowLeft is still held', () => {
    const { keyboardTarget, controller, sentFrames } = createController();

    dispatchKey(keyboardTarget, 'keydown', { code: 'MetaLeft', key: 'Meta', metaKey: true });
    dispatchKey(keyboardTarget, 'keydown', {
      code: 'ShiftLeft', key: 'Shift', shiftKey: true, metaKey: true,
    });
    dispatchKey(keyboardTarget, 'keydown', {
      code: 'ArrowLeft', key: 'ArrowLeft', shiftKey: true, metaKey: true,
    });
    dispatchKey(keyboardTarget, 'keydown', {
      code: 'ArrowLeft', key: 'ArrowLeft', repeat: true, metaKey: true,
    });
    dispatchKey(keyboardTarget, 'keyup', {
      code: 'ShiftLeft', key: 'Shift', metaKey: true,
    });
    dispatchKey(keyboardTarget, 'keyup', {
      code: 'ArrowLeft', key: 'ArrowLeft', metaKey: true,
    });
    dispatchKey(keyboardTarget, 'keyup', { code: 'MetaLeft', key: 'Meta' });

    controller.destroy();

    const keyFrames = decodeKeyFrames(sentFrames);
    expect(keyFrames.map(({ keycode, down, modifiers }) => ({
      keycode,
      down,
      modifiers,
    }))).toEqual([
      { keycode: 42, down: true, modifiers: 0x05 },
      { keycode: 102, down: true, modifiers: 0x04 },
      { keycode: 102, down: false, modifiers: 0x04 },
      { keycode: 42, down: false, modifiers: 0x01 },
    ]);
  });

  it('keeps Cmd+Shift+ArrowLeft atomic when Shift is released before the arrow keyup', () => {
    const { keyboardTarget, controller, sentFrames } = createController();

    dispatchKey(keyboardTarget, 'keydown', { code: 'MetaLeft', key: 'Meta', metaKey: true });
    dispatchKey(keyboardTarget, 'keydown', {
      code: 'ShiftLeft', key: 'Shift', shiftKey: true, metaKey: true,
    });
    dispatchKey(keyboardTarget, 'keydown', {
      code: 'ArrowLeft', key: 'ArrowLeft', shiftKey: true, metaKey: true,
    });
    dispatchKey(keyboardTarget, 'keyup', {
      code: 'ShiftLeft', key: 'Shift', metaKey: true,
    });
    dispatchKey(keyboardTarget, 'keyup', {
      code: 'ArrowLeft', key: 'ArrowLeft', metaKey: true,
    });
    dispatchKey(keyboardTarget, 'keyup', { code: 'MetaLeft', key: 'Meta' });

    controller.destroy();

    const keyFrames = decodeKeyFrames(sentFrames);
    expect(keyFrames.map(({ keycode, down, modifiers }) => ({
      keycode,
      down,
      modifiers,
    }))).toEqual([
      { keycode: 42, down: true, modifiers: 0x05 },
      { keycode: 102, down: true, modifiers: 0x04 },
      { keycode: 102, down: false, modifiers: 0x04 },
      { keycode: 42, down: false, modifiers: 0x01 },
    ]);
  });

  it('releases and restores the matching control key for right-command remaps', () => {
    const { keyboardTarget, controller, sentFrames } = createController();

    dispatchKey(keyboardTarget, 'keydown', { code: 'MetaRight', key: 'Meta', metaKey: true });
    dispatchKey(keyboardTarget, 'keydown', { code: 'KeyA', key: 'a', metaKey: true });
    dispatchKey(keyboardTarget, 'keyup', { code: 'KeyA', key: 'a', metaKey: true });
    dispatchKey(keyboardTarget, 'keydown', { code: 'ArrowRight', key: 'ArrowRight', metaKey: true });
    dispatchKey(keyboardTarget, 'keyup', { code: 'ArrowRight', key: 'ArrowRight', metaKey: true });
    dispatchKey(keyboardTarget, 'keyup', { code: 'MetaRight', key: 'Meta' });

    controller.destroy();

    const keyFrames = decodeKeyFrames(sentFrames);
    expect(keyFrames.map(({ keycode, down }) => ({ keycode, down }))).toEqual([
      { keycode: 97, down: true },
      { keycode: 30, down: true },
      { keycode: 30, down: false },
      { keycode: 97, down: false },
      { keycode: 107, down: true },
      { keycode: 107, down: false },
      { keycode: 97, down: true },
      { keycode: 97, down: false },
    ]);
    expect(keyFrames.some(({ keycode }) => keycode === 125 || keycode === 126)).toBe(false);
  });

  it('treats Option+L as character composition instead of a raw Alt shortcut', () => {
    const { keyboardTarget, controller, sentFrames } = createController();

    dispatchKey(keyboardTarget, 'keydown', { code: 'AltLeft', key: 'Alt', altKey: true });
    dispatchKey(keyboardTarget, 'keydown', { code: 'KeyL', key: '@', altKey: true });
    dispatchKey(keyboardTarget, 'keyup', { code: 'KeyL', key: '@', altKey: true });
    dispatchKey(keyboardTarget, 'keyup', { code: 'AltLeft', key: 'Alt' });

    controller.destroy();

    const keyFrames = decodeKeyFrames(sentFrames);
    expect(keyFrames.map(({ keycode, down }) => ({ keycode, down }))).toEqual([
      { keycode: 38, down: true },
      { keycode: 38, down: false },
    ]);
    expect(keyFrames[0]?.modifiers).toBe(0x10);
    expect(keyFrames[0]?.keyChar).toBe('@'.codePointAt(0));
    expect(keyFrames.some(({ keycode }) => keycode === 56 || keycode === 100)).toBe(false);
  });

  it('still materializes Option as Alt for non-text shortcuts', () => {
    const { keyboardTarget, controller, sentFrames } = createController();

    dispatchKey(keyboardTarget, 'keydown', { code: 'AltLeft', key: 'Alt', altKey: true });
    dispatchKey(keyboardTarget, 'keydown', { code: 'ArrowLeft', key: 'ArrowLeft', altKey: true });
    dispatchKey(keyboardTarget, 'keyup', { code: 'ArrowLeft', key: 'ArrowLeft', altKey: true });
    dispatchKey(keyboardTarget, 'keyup', { code: 'AltLeft', key: 'Alt' });

    controller.destroy();

    const keyFrames = decodeKeyFrames(sentFrames);
    expect(keyFrames.map(({ keycode, down }) => ({ keycode, down }))).toEqual([
      { keycode: 56, down: true },
      { keycode: 105, down: true },
      { keycode: 105, down: false },
      { keycode: 56, down: false },
    ]);
  });

  it('emits only the composed character for Option dead-key sequences', () => {
    const { keyboardTarget, controller, sentFrames } = createController();

    dispatchKey(keyboardTarget, 'keydown', { code: 'AltLeft', key: 'Alt', altKey: true });
    dispatchKey(keyboardTarget, 'keydown', { code: 'KeyE', key: 'Dead', altKey: true });
    dispatchKey(keyboardTarget, 'keyup', { code: 'KeyE', key: 'Dead', altKey: true });
    dispatchKey(keyboardTarget, 'keyup', { code: 'AltLeft', key: 'Alt' });
    dispatchKey(keyboardTarget, 'keydown', { code: 'KeyE', key: 'e' });
    dispatchInput(keyboardTarget, 'é');
    dispatchKey(keyboardTarget, 'keyup', { code: 'KeyE', key: 'e' });

    controller.destroy();

    const keyFrames = decodeKeyFrames(sentFrames);
    expect(keyFrames.map(({ keycode, down }) => ({ keycode, down }))).toEqual([
      { keycode: 18, down: true },
      { keycode: 18, down: false },
    ]);
    expect(keyFrames[0]?.keyChar).toBe('é'.codePointAt(0));
    expect(keyFrames.some(({ keycode }) => keycode === 56 || keycode === 100)).toBe(false);
  });

  it('still emits the composed character when compositionend arrives after keyup', async () => {
    vi.useFakeTimers();
    const { keyboardTarget, controller, sentFrames } = createController();

    dispatchKey(keyboardTarget, 'keydown', { code: 'AltLeft', key: 'Alt', altKey: true });
    dispatchKey(keyboardTarget, 'keydown', { code: 'KeyI', key: 'Dead', altKey: true });
    dispatchKey(keyboardTarget, 'keyup', { code: 'KeyI', key: 'Dead', altKey: true });
    dispatchKey(keyboardTarget, 'keyup', { code: 'AltLeft', key: 'Alt' });
    dispatchKey(keyboardTarget, 'keydown', {
      code: 'KeyO',
      key: 'Process',
      isComposing: true,
    });
    dispatchKey(keyboardTarget, 'keyup', { code: 'KeyO', key: 'o' });
    dispatchCompositionEnd(keyboardTarget, 'ô');
    await vi.runAllTimersAsync();

    controller.destroy();

    const keyFrames = decodeKeyFrames(sentFrames);
    expect(keyFrames.map(({ keycode, down }) => ({ keycode, down }))).toEqual([
      { keycode: 24, down: true },
      { keycode: 24, down: false },
    ]);
    expect(keyFrames[0]?.keyChar).toBe('ô'.codePointAt(0));
    expect(keyFrames.some(({ keycode }) => keycode === 56 || keycode === 100)).toBe(false);
  });

  it('does not depend on browser text events for supported dead keys', async () => {
    vi.useFakeTimers();
    const { keyboardTarget, controller, sentFrames } = createController();

    dispatchKey(keyboardTarget, 'keydown', { code: 'AltLeft', key: 'Alt', altKey: true });
    dispatchKey(keyboardTarget, 'keydown', { code: 'KeyE', key: 'Dead', altKey: true });
    dispatchKey(keyboardTarget, 'keyup', { code: 'KeyE', key: 'Dead', altKey: true });
    dispatchKey(keyboardTarget, 'keyup', { code: 'AltLeft', key: 'Alt' });
    dispatchKey(keyboardTarget, 'keydown', { code: 'KeyE', key: 'e' });
    dispatchKey(keyboardTarget, 'keyup', { code: 'KeyE', key: 'e' });
    await vi.runAllTimersAsync();

    controller.destroy();

    const keyFrames = decodeKeyFrames(sentFrames);
    expect(keyFrames.map(({ keycode, down }) => ({ keycode, down }))).toEqual([
      { keycode: 18, down: true },
      { keycode: 18, down: false },
    ]);
    expect(keyFrames[0]?.keyChar).toBe('é'.codePointAt(0));
  });

  it('ignores repeated base-key keydowns while waiting for composition text', async () => {
    vi.useFakeTimers();
    const { keyboardTarget, controller, sentFrames } = createController();

    dispatchKey(keyboardTarget, 'keydown', { code: 'AltLeft', key: 'Alt', altKey: true });
    dispatchKey(keyboardTarget, 'keydown', { code: 'KeyE', key: 'Dead', altKey: true });
    dispatchKey(keyboardTarget, 'keyup', { code: 'KeyE', key: 'Dead', altKey: true });
    dispatchKey(keyboardTarget, 'keyup', { code: 'AltLeft', key: 'Alt' });
    dispatchKey(keyboardTarget, 'keydown', { code: 'KeyE', key: 'e' });
    dispatchKey(keyboardTarget, 'keydown', { code: 'KeyE', key: 'e', repeat: true });
    dispatchKey(keyboardTarget, 'keyup', { code: 'KeyE', key: 'e' });
    dispatchCompositionEnd(keyboardTarget, 'é');
    await vi.runAllTimersAsync();

    controller.destroy();

    const keyFrames = decodeKeyFrames(sentFrames);
    expect(keyFrames.map(({ keycode, down }) => ({ keycode, down }))).toEqual([
      { keycode: 18, down: true },
      { keycode: 18, down: false },
    ]);
    expect(keyFrames[0]?.keyChar).toBe('é'.codePointAt(0));
  });

  it('synthesizes acute accents from the dedicated dead key on mac layouts', () => {
    const { keyboardTarget, controller, sentFrames } = createController();

    dispatchKey(keyboardTarget, 'keydown', { code: 'Equal', key: 'Dead' });
    dispatchKey(keyboardTarget, 'keyup', { code: 'Equal', key: 'Dead' });
    dispatchKey(keyboardTarget, 'keydown', { code: 'KeyE', key: 'e' });
    dispatchKey(keyboardTarget, 'keyup', { code: 'KeyE', key: 'e' });

    controller.destroy();

    const keyFrames = decodeKeyFrames(sentFrames);
    expect(keyFrames.map(({ keycode, down }) => ({ keycode, down }))).toEqual([
      { keycode: 18, down: true },
      { keycode: 18, down: false },
    ]);
    expect(keyFrames[0]?.keyChar).toBe('é'.codePointAt(0));
  });

  it('emits the accent even when the dead key is still held during the vowel release', () => {
    const { keyboardTarget, controller, sentFrames } = createController();

    dispatchKey(keyboardTarget, 'keydown', { code: 'Equal', key: 'Dead' });
    dispatchKey(keyboardTarget, 'keydown', { code: 'KeyE', key: 'e' });
    dispatchKey(keyboardTarget, 'keyup', { code: 'KeyE', key: 'e' });
    dispatchKey(keyboardTarget, 'keyup', { code: 'Equal', key: 'Dead' });

    controller.destroy();

    const keyFrames = decodeKeyFrames(sentFrames);
    expect(keyFrames.map(({ keycode, down }) => ({ keycode, down }))).toEqual([
      { keycode: 18, down: true },
      { keycode: 18, down: false },
    ]);
    expect(keyFrames[0]?.keyChar).toBe('é'.codePointAt(0));
  });

  it('synthesizes grave and circumflex accents from dedicated dead keys', () => {
    const { keyboardTarget, controller, sentFrames } = createController();

    dispatchKey(keyboardTarget, 'keydown', { code: 'Equal', key: 'Dead', shiftKey: true });
    dispatchKey(keyboardTarget, 'keyup', { code: 'Equal', key: 'Dead', shiftKey: true });
    dispatchKey(keyboardTarget, 'keydown', { code: 'KeyA', key: 'A', shiftKey: true });
    dispatchKey(keyboardTarget, 'keyup', { code: 'KeyA', key: 'A', shiftKey: true });

    dispatchKey(keyboardTarget, 'keydown', { code: 'Backquote', key: 'Dead' });
    dispatchKey(keyboardTarget, 'keyup', { code: 'Backquote', key: 'Dead' });
    dispatchKey(keyboardTarget, 'keydown', { code: 'KeyO', key: 'o' });
    dispatchKey(keyboardTarget, 'keyup', { code: 'KeyO', key: 'o' });

    controller.destroy();

    const keyFrames = decodeKeyFrames(sentFrames);
    expect(keyFrames.map(({ keycode, down, keyChar }) => ({
      keycode,
      down,
      keyChar,
    }))).toEqual([
      { keycode: 30, down: true, keyChar: 'À'.codePointAt(0) },
      { keycode: 30, down: false, keyChar: 'À'.codePointAt(0) },
      { keycode: 24, down: true, keyChar: 'ô'.codePointAt(0) },
      { keycode: 24, down: false, keyChar: 'ô'.codePointAt(0) },
    ]);
  });

  it('accepts Shift+Digit6 as a circumflex dead key on international layouts', () => {
    const { keyboardTarget, controller, sentFrames } = createController();

    dispatchKey(keyboardTarget, 'keydown', { code: 'Digit6', key: 'Dead', shiftKey: true });
    dispatchKey(keyboardTarget, 'keyup', { code: 'Digit6', key: 'Dead', shiftKey: true });
    dispatchKey(keyboardTarget, 'keydown', { code: 'KeyO', key: 'o' });
    dispatchKey(keyboardTarget, 'keyup', { code: 'KeyO', key: 'o' });

    dispatchKey(keyboardTarget, 'keydown', { code: 'Digit6', key: 'Dead', shiftKey: true });
    dispatchKey(keyboardTarget, 'keyup', { code: 'Digit6', key: 'Dead', shiftKey: true });
    dispatchKey(keyboardTarget, 'keydown', { code: 'Space', key: ' ' });
    dispatchKey(keyboardTarget, 'keyup', { code: 'Space', key: ' ' });

    controller.destroy();

    const keyFrames = decodeKeyFrames(sentFrames);
    expect(keyFrames.map(({ keycode, down, keyChar }) => ({
      keycode,
      down,
      keyChar,
    }))).toEqual([
      { keycode: 24, down: true, keyChar: 'ô'.codePointAt(0) },
      { keycode: 24, down: false, keyChar: 'ô'.codePointAt(0) },
      { keycode: 57, down: true, keyChar: '^'.codePointAt(0) },
      { keycode: 57, down: false, keyChar: '^'.codePointAt(0) },
    ]);
  });

  it('accepts IntlBackslash as a circumflex dead key on mac ISO layouts', () => {
    const { keyboardTarget, controller, sentFrames } = createController();

    dispatchKey(keyboardTarget, 'keydown', { code: 'IntlBackslash', key: 'Dead' });
    dispatchKey(keyboardTarget, 'keyup', { code: 'IntlBackslash', key: 'Dead' });
    dispatchKey(keyboardTarget, 'keydown', { code: 'KeyO', key: 'o' });
    dispatchKey(keyboardTarget, 'keyup', { code: 'KeyO', key: 'o' });

    dispatchKey(keyboardTarget, 'keydown', { code: 'IntlBackslash', key: 'Dead' });
    dispatchKey(keyboardTarget, 'keyup', { code: 'IntlBackslash', key: 'Dead' });
    dispatchKey(keyboardTarget, 'keydown', { code: 'Space', key: ' ' });
    dispatchKey(keyboardTarget, 'keyup', { code: 'Space', key: ' ' });

    controller.destroy();

    const keyFrames = decodeKeyFrames(sentFrames);
    expect(keyFrames.map(({ keycode, down, keyChar }) => ({
      keycode,
      down,
      keyChar,
    }))).toEqual([
      { keycode: 24, down: true, keyChar: 'ô'.codePointAt(0) },
      { keycode: 24, down: false, keyChar: 'ô'.codePointAt(0) },
      { keycode: 57, down: true, keyChar: '^'.codePointAt(0) },
      { keycode: 57, down: false, keyChar: '^'.codePointAt(0) },
    ]);
  });

  it('emits standalone accent characters for dedicated dead-key plus space', () => {
    const { keyboardTarget, controller, sentFrames } = createController();

    dispatchKey(keyboardTarget, 'keydown', { code: 'Equal', key: 'Dead' });
    dispatchKey(keyboardTarget, 'keyup', { code: 'Equal', key: 'Dead' });
    dispatchKey(keyboardTarget, 'keydown', { code: 'Space', key: ' ' });
    dispatchKey(keyboardTarget, 'keyup', { code: 'Space', key: ' ' });

    dispatchKey(keyboardTarget, 'keydown', { code: 'Equal', key: 'Dead', shiftKey: true });
    dispatchKey(keyboardTarget, 'keyup', { code: 'Equal', key: 'Dead', shiftKey: true });
    dispatchKey(keyboardTarget, 'keydown', { code: 'Space', key: ' ' });
    dispatchKey(keyboardTarget, 'keyup', { code: 'Space', key: ' ' });

    dispatchKey(keyboardTarget, 'keydown', { code: 'Backquote', key: 'Dead' });
    dispatchKey(keyboardTarget, 'keyup', { code: 'Backquote', key: 'Dead' });
    dispatchKey(keyboardTarget, 'keydown', { code: 'Space', key: ' ' });
    dispatchKey(keyboardTarget, 'keyup', { code: 'Space', key: ' ' });

    controller.destroy();

    const keyFrames = decodeKeyFrames(sentFrames);
    expect(keyFrames.map(({ keycode, down, keyChar }) => ({
      keycode,
      down,
      keyChar,
    }))).toEqual([
      { keycode: 57, down: true, keyChar: '´'.codePointAt(0) },
      { keycode: 57, down: false, keyChar: '´'.codePointAt(0) },
      { keycode: 57, down: true, keyChar: '`'.codePointAt(0) },
      { keycode: 57, down: false, keyChar: '`'.codePointAt(0) },
      { keycode: 57, down: true, keyChar: '^'.codePointAt(0) },
      { keycode: 57, down: false, keyChar: '^'.codePointAt(0) },
    ]);
  });

  it('emits standalone accent characters for Option dead-key plus space', () => {
    const { keyboardTarget, controller, sentFrames } = createController();

    dispatchKey(keyboardTarget, 'keydown', { code: 'AltLeft', key: 'Alt', altKey: true });
    dispatchKey(keyboardTarget, 'keydown', { code: 'KeyE', key: 'Dead', altKey: true });
    dispatchKey(keyboardTarget, 'keyup', { code: 'KeyE', key: 'Dead', altKey: true });
    dispatchKey(keyboardTarget, 'keyup', { code: 'AltLeft', key: 'Alt' });
    dispatchKey(keyboardTarget, 'keydown', { code: 'Space', key: ' ' });
    dispatchKey(keyboardTarget, 'keyup', { code: 'Space', key: ' ' });

    controller.destroy();

    const keyFrames = decodeKeyFrames(sentFrames);
    expect(keyFrames.map(({ keycode, down, keyChar }) => ({
      keycode,
      down,
      keyChar,
    }))).toEqual([
      { keycode: 57, down: true, keyChar: '´'.codePointAt(0) },
      { keycode: 57, down: false, keyChar: '´'.codePointAt(0) },
    ]);
  });

  it('keeps emitting accents across repeated dedicated dead-key attempts', () => {
    const { keyboardTarget, controller, sentFrames } = createController();

    for (let i = 0; i < 3; i += 1) {
      dispatchKey(keyboardTarget, 'keydown', { code: 'Equal', key: 'Dead' });
      dispatchKey(keyboardTarget, 'keyup', { code: 'Equal', key: 'Dead' });
      dispatchKey(keyboardTarget, 'keydown', { code: 'KeyE', key: 'e' });
      dispatchKey(keyboardTarget, 'keyup', { code: 'KeyE', key: 'e' });
    }

    controller.destroy();

    const keyFrames = decodeKeyFrames(sentFrames);
    expect(keyFrames.map(({ keycode, down, keyChar }) => ({
      keycode,
      down,
      keyChar,
    }))).toEqual([
      { keycode: 18, down: true, keyChar: 'é'.codePointAt(0) },
      { keycode: 18, down: false, keyChar: 'é'.codePointAt(0) },
      { keycode: 18, down: true, keyChar: 'é'.codePointAt(0) },
      { keycode: 18, down: false, keyChar: 'é'.codePointAt(0) },
      { keycode: 18, down: true, keyChar: 'é'.codePointAt(0) },
      { keycode: 18, down: false, keyChar: 'é'.codePointAt(0) },
    ]);
  });

  it('falls back to the spacing accent for unsupported dead-key pairings', () => {
    const { keyboardTarget, controller, sentFrames } = createController();

    dispatchKey(keyboardTarget, 'keydown', { code: 'Equal', key: 'Dead' });
    dispatchKey(keyboardTarget, 'keydown', { code: 'KeyY', key: 'y' });
    dispatchKey(keyboardTarget, 'keyup', { code: 'KeyY', key: 'y' });
    dispatchKey(keyboardTarget, 'keyup', { code: 'Equal', key: 'Dead' });

    controller.destroy();

    const keyFrames = decodeKeyFrames(sentFrames);
    expect(keyFrames.map(({ keycode, down, keyChar }) => ({
      keycode,
      down,
      keyChar,
    }))).toEqual([
      { keycode: 13, down: true, keyChar: '´'.codePointAt(0) },
      { keycode: 13, down: false, keyChar: '´'.codePointAt(0) },
      { keycode: 21, down: true, keyChar: 'y'.codePointAt(0) },
      { keycode: 21, down: false, keyChar: 'y'.codePointAt(0) },
    ]);
  });
});

describe('InputController Windows paste handling', () => {
  it('waits for clipboard sync before Ctrl+V and delays Ctrl release until after V', async () => {
    setPlatform('Win32', 'Windows');

    let resolveClipboard = (_text: string) => {};
    const readText = vi.fn(() => new Promise<string>((resolve) => {
      resolveClipboard = resolve;
    }));
    Object.defineProperty(navigator, 'clipboard', {
      value: {
        readText,
        writeText: vi.fn().mockResolvedValue(undefined),
      },
      configurable: true,
    });

    const { keyboardTarget, controller, sentFrames } = createController({ clipboardEnabled: true });

    dispatchKey(keyboardTarget, 'keydown', { code: 'ControlLeft', key: 'Control', ctrlKey: true });
    dispatchKey(keyboardTarget, 'keydown', { code: 'KeyV', key: 'v', ctrlKey: true });
    dispatchKey(keyboardTarget, 'keyup', { code: 'ControlLeft', key: 'Control' });
    dispatchKey(keyboardTarget, 'keyup', { code: 'KeyV', key: 'v' });

    let inputFrames = decodeKeyFrames(sentFrames.filter(({ channelId }) => channelId === CH_INPUT));
    expect(inputFrames.map(({ keycode, down }) => ({ keycode, down }))).toEqual([
      { keycode: 29, down: true },
    ]);

    resolveClipboard('win paste');
    await Promise.resolve();
    await Promise.resolve();

    controller.destroy();

    expect(readText).toHaveBeenCalledTimes(1);
    expect(sentFrames.map(({ channelId }) => channelId)).toEqual([
      CH_INPUT,
      CH_CLIPBOARD,
      CH_INPUT,
      CH_INPUT,
      CH_INPUT,
    ]);
    expect(decodeClipboardText(sentFrames[1])).toBe('win paste');

    inputFrames = decodeKeyFrames(sentFrames.filter(({ channelId }) => channelId === CH_INPUT));
    expect(inputFrames.map(({ keycode, down, modifiers, keyChar }) => ({
      keycode,
      down,
      modifiers,
      keyChar,
    }))).toEqual([
      { keycode: 29, down: true, modifiers: 1, keyChar: 0 },
      { keycode: 47, down: true, modifiers: 1, keyChar: 0 },
      { keycode: 47, down: false, modifiers: 1, keyChar: 0 },
      { keycode: 29, down: false, modifiers: 0, keyChar: 0 },
    ]);
  });
});

describe('InputController shared wire fixtures', () => {
  it('emits the shared key-event-ex fixture for a simple printable keydown', () => {
    setPlatform('Linux x86_64', 'Linux');
    const { keyboardTarget, controller, sentFrames } = createController();

    dispatchKey(keyboardTarget, 'keydown', { code: 'KeyA', key: 'a' });

    controller.destroy();

    expect(sentFrames).toHaveLength(1);
    expect(encodeFrame(sentFrames[0].channelId, sentFrames[0].payload)).toEqual(
      wireFixture('input_key_event_ex'),
    );
  });

  it('emits the shared clipboard fixture', () => {
    setPlatform('Linux x86_64', 'Linux');
    const { controller, sentFrames } = createController({ clipboardEnabled: true });

    controller.sendClipboardText('hello clipboard');
    controller.destroy();

    expect(sentFrames).toHaveLength(1);
    expect(encodeFrame(sentFrames[0].channelId, sentFrames[0].payload)).toEqual(
      wireFixture('clipboard_text'),
    );
  });
});
