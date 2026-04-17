import { describe, expect, it, vi } from 'vitest';
import {
  inferLayoutHint,
  inferLayoutName,
  sendKeyboardLayoutHint,
  type NavigatorLike,
} from '../input/layout-hint.js';

type LayoutChangeListener = () => void;

async function flushPromises(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

function createKeyboardApi(options: {
  layoutMaps?: Array<Map<string, string>>;
  rejectInitial?: boolean;
  rejectOnLayoutChange?: boolean;
}) {
  const listeners = new Map<string, LayoutChangeListener[]>();
  let requestCount = 0;

  const getLayoutMap = vi.fn(async () => {
    requestCount += 1;
    if (options.rejectInitial && requestCount === 1) {
      throw new Error('initial failure');
    }
    if (options.rejectOnLayoutChange && requestCount > 1) {
      throw new Error('layoutchange failure');
    }
    const index = Math.min(
      requestCount - 1,
      (options.layoutMaps?.length ?? 1) - 1,
    );
    return options.layoutMaps?.[index] ?? new Map<string, string>();
  });

  return {
    getLayoutMap,
    addEventListener(type: string, listener: LayoutChangeListener) {
      const handlers = listeners.get(type) ?? [];
      handlers.push(listener);
      listeners.set(type, handlers);
    },
    dispatch(type: string) {
      for (const listener of listeners.get(type) ?? []) {
        listener();
      }
    },
  };
}

function createLayoutMap(layout: 'us' | 'de', physical: 'ansi' | 'iso' = 'ansi') {
  const map = new Map<string, string>();
  if (layout === 'us') {
    map.set('KeyQ', 'q');
    map.set('KeyW', 'w');
    map.set('KeyY', 'y');
    map.set('KeyZ', 'z');
  } else {
    map.set('KeyQ', 'q');
    map.set('KeyW', 'w');
    map.set('KeyY', 'z');
    map.set('KeyZ', 'y');
  }
  if (physical === 'iso') {
    map.set('IntlBackslash', '<');
  }
  return map;
}

describe('layout hint helpers', () => {
  it('infers layout names from the layout map', () => {
    expect(inferLayoutName(createLayoutMap('us'))).toBe('us');
    expect(inferLayoutName(createLayoutMap('de'))).toBe('de');
    expect(inferLayoutName(new Map())).toBe('');
  });

  it('adds physical and OS metadata to the layout hint', () => {
    expect(inferLayoutHint(createLayoutMap('us', 'iso'), {
      platform: 'Linux x86_64',
      userAgentData: { platform: 'Linux' },
    })).toBe('us-iso-linux');
  });

  it('sends an empty hint when the keyboard api is missing', () => {
    const sendHint = vi.fn();

    sendKeyboardLayoutHint({
      navigatorLike: {},
      sendHint,
    });

    expect(sendHint).toHaveBeenCalledWith('');
  });

  it('sends the initial layout hint and refreshes on layout changes', async () => {
    const sendHint = vi.fn();
    const keyboard = createKeyboardApi({
      layoutMaps: [
        createLayoutMap('us', 'ansi'),
        createLayoutMap('de', 'iso'),
      ],
    });
    const navigatorLike: NavigatorLike = {
      platform: 'MacIntel',
      userAgentData: { platform: 'macOS' },
      keyboard,
    };

    sendKeyboardLayoutHint({
      navigatorLike,
      sendHint,
    });
    await flushPromises();

    keyboard.dispatch('layoutchange');
    await flushPromises();

    expect(sendHint).toHaveBeenNthCalledWith(1, 'us-ansi-mac');
    expect(sendHint).toHaveBeenNthCalledWith(2, 'de-iso-mac');
  });

  it('falls back to an empty initial hint when getLayoutMap rejects', async () => {
    const sendHint = vi.fn();
    const keyboard = createKeyboardApi({
      rejectInitial: true,
    });

    sendKeyboardLayoutHint({
      navigatorLike: {
        platform: 'Win32',
        userAgentData: { platform: 'Windows' },
        keyboard,
      },
      sendHint,
    });
    await flushPromises();

    expect(sendHint).toHaveBeenCalledTimes(1);
    expect(sendHint).toHaveBeenCalledWith('');
  });

  it('ignores layoutchange failures after a successful initial send', async () => {
    const sendHint = vi.fn();
    const keyboard = createKeyboardApi({
      layoutMaps: [createLayoutMap('us')],
      rejectOnLayoutChange: true,
    });

    sendKeyboardLayoutHint({
      navigatorLike: {
        platform: 'Win32',
        userAgentData: { platform: 'Windows' },
        keyboard,
      },
      sendHint,
    });
    await flushPromises();

    keyboard.dispatch('layoutchange');
    await flushPromises();

    expect(sendHint).toHaveBeenCalledTimes(1);
    expect(sendHint).toHaveBeenCalledWith('us-ansi-win');
  });
});
