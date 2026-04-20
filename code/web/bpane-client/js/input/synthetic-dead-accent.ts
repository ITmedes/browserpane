export type SupportedDeadAccent = 'acute' | 'grave' | 'circumflex';

type SyntheticDeadAccentBaseKey =
  | 'a'
  | 'e'
  | 'i'
  | 'o'
  | 'u'
  | 'A'
  | 'E'
  | 'I'
  | 'O'
  | 'U'
  | 'Space';

type KeyEventLike = {
  code: string;
  key: string;
  altKey: boolean;
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
};

const MAC_CIRCUMFLEX_DEAD_CODES = new Set(['Backquote', 'Digit6', 'IntlBackslash']);

const SYNTHETIC_DEAD_ACCENT_MAP: Record<
SupportedDeadAccent,
  Record<SyntheticDeadAccentBaseKey, string>
> = {
  acute: {
    a: 'á', e: 'é', i: 'í', o: 'ó', u: 'ú',
    A: 'Á', E: 'É', I: 'Í', O: 'Ó', U: 'Ú',
    Space: '´',
  },
  grave: {
    a: 'à', e: 'è', i: 'ì', o: 'ò', u: 'ù',
    A: 'À', E: 'È', I: 'Ì', O: 'Ò', U: 'Ù',
    Space: '`',
  },
  circumflex: {
    a: 'â', e: 'ê', i: 'î', o: 'ô', u: 'û',
    A: 'Â', E: 'Ê', I: 'Î', O: 'Ô', U: 'Û',
    Space: '^',
  },
};

export function getSyntheticDeadAccentSpacingCharacter(accent: SupportedDeadAccent): string {
  return SYNTHETIC_DEAD_ACCENT_MAP[accent].Space;
}

export function resolveSupportedDeadAccent(
  event: KeyEventLike,
  isMac: boolean,
): SupportedDeadAccent | null {
  if (!isMac || event.key !== 'Dead' || event.ctrlKey || event.metaKey) {
    return null;
  }

  if (event.altKey) {
    switch (event.code) {
      case 'KeyE':
        return 'acute';
      case 'KeyI':
        return 'circumflex';
      case 'Backquote':
        return 'grave';
      default:
        return null;
    }
  }

  switch (event.code) {
    case 'Equal':
      return event.shiftKey ? 'grave' : 'acute';
    default:
      if (MAC_CIRCUMFLEX_DEAD_CODES.has(event.code)) {
        return 'circumflex';
      }
      return null;
  }
}

export function composeSyntheticDeadAccent(
  accent: SupportedDeadAccent,
  event: KeyEventLike,
): string | null {
  const normalizedKey = normalizeSyntheticAccentBaseKey(event);
  if (!normalizedKey) {
    return null;
  }
  return SYNTHETIC_DEAD_ACCENT_MAP[accent][normalizedKey] ?? null;
}

function normalizeSyntheticAccentBaseKey(event: KeyEventLike): SyntheticDeadAccentBaseKey | null {
  if (event.key === ' ' || event.code === 'Space') {
    return 'Space';
  }

  if (event.key.length === 1) {
    const lower = event.key.toLowerCase();
    if ('aeiou'.includes(lower)) {
      return event.key === lower
        ? lower as SyntheticDeadAccentBaseKey
        : lower.toUpperCase() as SyntheticDeadAccentBaseKey;
    }
  }

  if (event.code.startsWith('Key')) {
    const lower = event.code.slice(3).toLowerCase();
    if ('aeiou'.includes(lower)) {
      return event.shiftKey
        ? lower.toUpperCase() as SyntheticDeadAccentBaseKey
        : lower as SyntheticDeadAccentBaseKey;
    }
  }

  return null;
}
