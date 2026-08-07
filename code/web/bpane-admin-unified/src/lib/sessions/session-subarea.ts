export type SessionSubareaId = 'overview' | 'live' | 'files' | 'recordings' | 'network';

export type SessionSubareaDefinition = {
  readonly id: SessionSubareaId;
  readonly label: string;
  readonly suffix: string;
};

export type SessionSubareaRoute = {
  readonly sessionId: string;
  readonly activeId: SessionSubareaId;
};

export const sessionSubareas: readonly SessionSubareaDefinition[] = [
  { id: 'overview', label: 'Overview', suffix: '' },
  { id: 'live', label: 'Live', suffix: '/live' },
  { id: 'files', label: 'Files', suffix: '/files' },
  { id: 'recordings', label: 'Recordings', suffix: '/recordings' },
  { id: 'network', label: 'Network', suffix: '/network' },
];

export function sessionSubareaHref(sessionId: string, subareaId: SessionSubareaId): string {
  const definition = sessionSubareas.find((candidate) => candidate.id === subareaId);
  if (!definition) {
    throw new Error(`Unknown session subarea: ${subareaId}`);
  }
  return `/admin-new/sessions/${encodeURIComponent(sessionId)}${definition.suffix}`;
}

export function resolveSessionSubareaRoute(pathname: string): SessionSubareaRoute | null {
  const normalized = normalizeRoute(pathname);
  const match = normalized.match(/^\/sessions\/([^/]+)(?:\/(live|files|recordings|network|preview))?$/);
  if (!match?.[1]) {
    return null;
  }
  const sessionId = safeDecode(match[1]);
  if (!sessionId) {
    return null;
  }
  const suffix = match[2] ?? '';
  return {
    sessionId,
    activeId: subareaIdFromSuffix(suffix),
  };
}

function subareaIdFromSuffix(suffix: string): SessionSubareaId {
  if (suffix === '') {
    return 'overview';
  }
  if (suffix === 'preview' || suffix === 'live') {
    return 'live';
  }
  if (suffix === 'files') {
    return 'files';
  }
  if (suffix === 'recordings') {
    return 'recordings';
  }
  return 'network';
}

function normalizeRoute(pathname: string): string {
  const withoutBase = pathname.replace(/^\/admin-new(?=\/|$)/, '');
  const normalized = `/${withoutBase}`.replace(/\/{2,}/g, '/').replace(/\/$/, '');
  return normalized || '/';
}

function safeDecode(value: string): string | null {
  try {
    return decodeURIComponent(value);
  } catch {
    return null;
  }
}
