export function resolveNewSessionId(previousSessionIds, currentSessionIds) {
  const previous = normalizedSessionIds(previousSessionIds);
  const added = [...normalizedSessionIds(currentSessionIds)].filter((sessionId) => !previous.has(sessionId));

  if (added.length === 0) {
    return '';
  }
  if (added.length > 1) {
    const sample = added.slice(0, 5);
    const remainder = added.length - sample.length;
    const suffix = remainder > 0 ? ` (+${remainder} more)` : '';
    throw new Error(`Expected exactly one newly created session, observed ${added.length}: ${sample.join(', ')}${suffix}`);
  }
  return added[0];
}

function normalizedSessionIds(sessionIds) {
  return new Set(sessionIds.filter((sessionId) => typeof sessionId === 'string' && sessionId.length > 0));
}
