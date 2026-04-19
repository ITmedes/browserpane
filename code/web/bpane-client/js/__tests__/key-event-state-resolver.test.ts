import { describe, expect, it } from 'vitest';
import { KeyEventStateResolver } from '../input/key-event-state-resolver.js';

function createEvent(input: {
  ctrlKey?: boolean;
  altKey?: boolean;
  metaKey?: boolean;
  shiftKey?: boolean;
  altGraph?: boolean;
}) {
  return {
    key: '',
    ctrlKey: input.ctrlKey ?? false,
    altKey: input.altKey ?? false,
    metaKey: input.metaKey ?? false,
    shiftKey: input.shiftKey ?? false,
    getModifierState(name: string): boolean {
      return name === 'AltGraph' ? (input.altGraph ?? false) : false;
    },
  };
}

describe('KeyEventStateResolver', () => {
  it('passes through ordinary modifier state', () => {
    const resolver = new KeyEventStateResolver({
      macMetaAsCtrl: false,
      isMacOptionComposition: () => false,
    });

    expect(resolver.resolve(createEvent({
      ctrlKey: true,
      altKey: true,
      shiftKey: true,
      metaKey: true,
    }))).toEqual({
      ctrl: true,
      alt: true,
      shift: true,
      meta: true,
      altgr: false,
    });
  });

  it('maps mac meta to control and removes meta for the host', () => {
    const resolver = new KeyEventStateResolver({
      macMetaAsCtrl: true,
      isMacOptionComposition: () => false,
    });

    expect(resolver.resolve(createEvent({
      metaKey: true,
      shiftKey: true,
    }))).toEqual({
      ctrl: true,
      alt: false,
      shift: true,
      meta: false,
      altgr: false,
    });
  });

  it('treats AltGraph as alt without control', () => {
    const resolver = new KeyEventStateResolver({
      macMetaAsCtrl: false,
      isMacOptionComposition: () => false,
    });

    expect(resolver.resolve(createEvent({
      ctrlKey: true,
      altKey: true,
      altGraph: true,
    }))).toEqual({
      ctrl: false,
      alt: true,
      shift: false,
      meta: false,
      altgr: true,
    });
  });

  it('treats mac option composition as altgr-style input instead of raw alt', () => {
    const resolver = new KeyEventStateResolver({
      macMetaAsCtrl: false,
      isMacOptionComposition: () => true,
    });

    expect(resolver.resolve(createEvent({
      altKey: true,
    }))).toEqual({
      ctrl: false,
      alt: false,
      shift: false,
      meta: false,
      altgr: true,
    });
  });
});
