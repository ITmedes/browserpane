import type {
  ShortcutKeyReleaseRuntime,
  ShortcutModifierKeyupEvent,
  ShortcutReleasedKey,
} from './shortcut-key-release-runtime.js';

export interface ShortcutKeyReleaseDispatcherInput {
  runtime: ShortcutKeyReleaseRuntime;
  emitKeyRelease: (release: ShortcutReleasedKey) => void;
  suppressKeyup: (code: string) => void;
}

export class ShortcutKeyReleaseDispatcher {
  private readonly runtime: ShortcutKeyReleaseRuntime;
  private readonly emitKeyRelease: (release: ShortcutReleasedKey) => void;
  private readonly suppressKeyup: (code: string) => void;

  constructor(input: ShortcutKeyReleaseDispatcherInput) {
    this.runtime = input.runtime;
    this.emitKeyRelease = input.emitKeyRelease;
    this.suppressKeyup = input.suppressKeyup;
  }

  releaseKeysForModifierKeyup(input: ShortcutModifierKeyupEvent): void {
    const releases = this.runtime.releaseKeysForModifierKeyup(input);
    for (const release of releases) {
      this.emitKeyRelease(release);
      this.suppressKeyup(release.code);
    }
  }
}
