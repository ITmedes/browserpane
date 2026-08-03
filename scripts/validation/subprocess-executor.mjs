import { spawn } from 'node:child_process';

const SIGNAL_EXIT_CODES = new Map([
  ['SIGHUP', 129],
  ['SIGINT', 130],
  ['SIGTERM', 143],
  ['SIGKILL', 137]
]);

export class SubprocessStageExecutor {
  #child = null;
  #killTimer = null;
  #requestedSignal = null;
  #killGraceMs;
  #stdio;

  constructor(options = {}) {
    this.#killGraceMs = options.killGraceMs ?? 2000;
    this.#stdio = options.stdio ?? 'inherit';
  }

  execute(stage) {
    return new Promise((resolve) => {
      let timedOut = false;
      let settled = false;
      const child = spawn(stage.command, stage.args, {
        cwd: stage.cwd,
        env: process.env,
        stdio: this.#stdio,
        detached: process.platform !== 'win32'
      });
      this.#child = child;

      const timeout = setTimeout(() => {
        timedOut = true;
        this.#terminate('SIGTERM');
      }, stage.timeoutMs);

      const finish = (exitCode, signal = null) => {
        if (settled) return;
        settled = true;
        clearTimeout(timeout);
        if (this.#killTimer) clearTimeout(this.#killTimer);
        const requestedSignal = this.#requestedSignal;
        this.#child = null;
        this.#killTimer = null;
        this.#requestedSignal = null;
        resolve({
          exitCode: timedOut ? 124 : exitCode ?? SIGNAL_EXIT_CODES.get(requestedSignal ?? signal) ?? 1,
          signal: requestedSignal ?? signal,
          timedOut
        });
      };

      child.once('error', () => finish(127));
      child.once('close', (code, signal) => finish(code, signal));
    });
  }

  cancel(signal = 'SIGTERM') {
    this.#requestedSignal = signal;
    this.#terminate(signal);
  }

  #terminate(signal) {
    const child = this.#child;
    if (!child?.pid) return;
    try {
      if (process.platform === 'win32') child.kill(signal);
      else process.kill(-child.pid, signal);
    } catch (error) {
      if (error.code !== 'ESRCH') throw error;
    }
    if (signal !== 'SIGKILL' && !this.#killTimer) {
      this.#killTimer = setTimeout(() => this.#terminate('SIGKILL'), this.#killGraceMs);
    }
  }
}
