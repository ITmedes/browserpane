const DEFAULT_TIMEOUT_MS = 15_000;
const DEFAULT_SETTLE_MS = 250;
const DEFAULT_QUIET_PASSES = 3;

export class SessionCleanup {
  #list;
  #kill;
  #timeoutMs;
  #settleMs;
  #quietPasses;
  #now;
  #wait;

  constructor({
    list,
    kill,
    timeoutMs = DEFAULT_TIMEOUT_MS,
    settleMs = DEFAULT_SETTLE_MS,
    quietPasses = DEFAULT_QUIET_PASSES,
    now = Date.now,
    wait = delay,
  }) {
    SessionCleanup.#validate({ list, kill, timeoutMs, settleMs, quietPasses, now, wait });
    this.#list = list;
    this.#kill = kill;
    this.#timeoutMs = timeoutMs;
    this.#settleMs = settleMs;
    this.#quietPasses = quietPasses;
    this.#now = now;
    this.#wait = wait;
  }

  async run() {
    let progressDeadline = this.#now() + this.#timeoutMs;
    const removedSessionIds = new Set();
    let consecutiveQuietPasses = 0;

    while (this.#now() <= progressDeadline) {
      const sessionIds = SessionCleanup.#activeSessionIds(await this.#list());
      if (sessionIds.length === 0) {
        consecutiveQuietPasses += 1;
        if (consecutiveQuietPasses >= this.#quietPasses) {
          return { removedSessionIds: [...removedSessionIds] };
        }
      } else {
        consecutiveQuietPasses = 0;
        for (const sessionId of sessionIds) {
          await this.#kill(sessionId);
          if (!removedSessionIds.has(sessionId)) {
            removedSessionIds.add(sessionId);
            progressDeadline = this.#now() + this.#timeoutMs;
          }
        }
      }

      if (this.#now() >= progressDeadline) break;
      await this.#wait(this.#settleMs);
    }

    throw new Error(
      `Session cleanup made no progress for ${this.#timeoutMs}ms before reaching a stable empty state.`,
    );
  }

  static #activeSessionIds(response) {
    if (!response || !Array.isArray(response.sessions)) {
      throw new Error('Session cleanup expected a session catalog response.');
    }
    return [...new Set(response.sessions.flatMap((session) => {
      const sessionId = typeof session?.id === 'string' ? session.id : '';
      const sessionState = typeof session?.state === 'string' ? session.state : '';
      return sessionId && sessionState && sessionState !== 'stopped' ? [sessionId] : [];
    }))];
  }

  static #validate({ list, kill, timeoutMs, settleMs, quietPasses, now, wait }) {
    if (typeof list !== 'function' || typeof kill !== 'function') {
      throw new TypeError('Session cleanup requires list and kill functions.');
    }
    if (typeof now !== 'function' || typeof wait !== 'function') {
      throw new TypeError('Session cleanup requires clock and wait functions.');
    }
    if (!Number.isInteger(timeoutMs) || timeoutMs <= 0) {
      throw new TypeError('Session cleanup timeoutMs must be a positive integer.');
    }
    if (!Number.isInteger(settleMs) || settleMs < 0) {
      throw new TypeError('Session cleanup settleMs must be a non-negative integer.');
    }
    if (!Number.isInteger(quietPasses) || quietPasses <= 0) {
      throw new TypeError('Session cleanup quietPasses must be a positive integer.');
    }
  }
}

function delay(durationMs) {
  return new Promise((resolve) => setTimeout(resolve, durationMs));
}
