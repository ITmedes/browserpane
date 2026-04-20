export interface LayoutMapLike {
  get(key: string): string | undefined;
  has(key: string): boolean;
}

export interface KeyboardApiLike {
  getLayoutMap?: () => Promise<LayoutMapLike>;
  addEventListener?: (type: 'layoutchange', listener: () => void) => void;
}

export interface NavigatorLike {
  platform?: string;
  userAgentData?: {
    platform?: string;
  };
  keyboard?: KeyboardApiLike;
}

function detectPhysicalLayout(map: LayoutMapLike): 'iso' | 'ansi' {
  return map.has('IntlBackslash') ? 'iso' : 'ansi';
}

function detectOsPlatform(navigatorLike: NavigatorLike): 'mac' | 'win' | 'linux' {
  if (navigatorLike.userAgentData?.platform === 'macOS' || navigatorLike.platform?.startsWith('Mac')) {
    return 'mac';
  }
  if (navigatorLike.platform?.startsWith('Linux')) {
    return 'linux';
  }
  return 'win';
}

export function inferLayoutName(map: LayoutMapLike): string {
  const q = map.get('KeyQ') ?? '';
  const w = map.get('KeyW') ?? '';
  const y = map.get('KeyY') ?? '';
  const z = map.get('KeyZ') ?? '';

  if (q === 'a' && w === 'z') {
    return 'fr';
  }
  if (z === 'y' && y === 'z') {
    return 'de';
  }
  if (q === 'q' && w === 'w' && z === 'z') {
    return 'us';
  }
  return '';
}

export function inferLayoutHint(
  map: LayoutMapLike,
  navigatorLike: NavigatorLike = navigator,
): string {
  const lang = inferLayoutName(map);
  if (!lang) {
    return '';
  }
  const physical = detectPhysicalLayout(map);
  const os = detectOsPlatform(navigatorLike);
  return `${lang}-${physical}-${os}`;
}

export function sendKeyboardLayoutHint(input: {
  navigatorLike?: NavigatorLike;
  sendHint: (hint: string) => void;
}): void {
  const navigatorLike = input.navigatorLike;
  if (!navigatorLike) {
    return;
  }

  const keyboard = navigatorLike.keyboard;
  if (!keyboard?.getLayoutMap) {
    input.sendHint('');
    return;
  }

  const sendCurrentHint = (fallbackToEmpty: boolean) => {
    keyboard.getLayoutMap?.().then((map) => {
      input.sendHint(inferLayoutHint(map, navigatorLike));
    }).catch(() => {
      if (fallbackToEmpty) {
        input.sendHint('');
      }
    });
  };

  sendCurrentHint(true);

  keyboard.addEventListener?.('layoutchange', () => {
    sendCurrentHint(false);
  });
}
