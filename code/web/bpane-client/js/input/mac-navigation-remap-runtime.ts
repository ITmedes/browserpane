export interface RemappedNavigationKeyState {
  releasedCtrlCodes: string[];
}

export interface MacNavigationRemapResult {
  remapCode: 'Home' | 'End';
}

export class MacNavigationRemapRuntime {
  private readonly remappedKeys = new Map<string, RemappedNavigationKeyState>();

  begin(code: string, releasedCtrlCodes: string[]): MacNavigationRemapResult | null {
    if (this.remappedKeys.has(code)) {
      return null;
    }

    switch (code) {
      case 'ArrowLeft':
        this.remappedKeys.set(code, { releasedCtrlCodes });
        return { remapCode: 'Home' };
      case 'ArrowRight':
        this.remappedKeys.set(code, { releasedCtrlCodes });
        return { remapCode: 'End' };
      default:
        return null;
    }
  }

  hasActiveRemap(code: string): boolean {
    return this.remappedKeys.has(code);
  }

  handleKeyup(code: string): RemappedNavigationKeyState | null {
    const remapped = this.remappedKeys.get(code) ?? null;
    if (remapped) {
      this.remappedKeys.delete(code);
    }
    return remapped;
  }

  reset(): void {
    this.remappedKeys.clear();
  }
}
