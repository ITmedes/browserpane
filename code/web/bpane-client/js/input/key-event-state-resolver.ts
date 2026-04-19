export interface KeyEventStateResolverEvent {
  key: string;
  ctrlKey: boolean;
  altKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
  getModifierState(name: string): boolean;
}

export interface ResolvedKeyEventState {
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  meta: boolean;
  altgr: boolean;
}

export interface KeyEventStateResolverInput {
  macMetaAsCtrl: boolean;
  isMacOptionComposition: (event: KeyEventStateResolverEvent) => boolean;
}

export class KeyEventStateResolver {
  private readonly macMetaAsCtrl: boolean;
  private readonly isMacOptionComposition: (event: KeyEventStateResolverEvent) => boolean;

  constructor(input: KeyEventStateResolverInput) {
    this.macMetaAsCtrl = input.macMetaAsCtrl;
    this.isMacOptionComposition = input.isMacOptionComposition;
  }

  resolve(event: KeyEventStateResolverEvent): ResolvedKeyEventState {
    let altgr = event.getModifierState('AltGraph');
    let ctrl = event.ctrlKey;
    let alt = event.altKey;
    const meta = event.metaKey;

    if (this.macMetaAsCtrl && meta) {
      ctrl = true;
    }

    if (altgr) {
      ctrl = false;
      alt = true;
    }

    if (this.isMacOptionComposition(event)) {
      altgr = true;
      alt = false;
    }

    return {
      ctrl,
      alt,
      shift: event.shiftKey,
      meta: meta && !this.macMetaAsCtrl,
      altgr,
    };
  }
}
