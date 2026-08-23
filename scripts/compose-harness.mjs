#!/usr/bin/env node

import { ComposeHarnessRecorder } from './compose-harness/harness-recorder.mjs';
import { ReadinessWaiter, boundaryReady } from './compose-harness/readiness-waiter.mjs';

class ComposeHarnessCommand {
  async run(argv) {
    const operation = argv.shift();
    if (operation !== 'wait-http') {
      throw new Error('usage: compose-harness.mjs wait-http --boundary <boundary> --url <url>');
    }
    const options = this.#parse(argv);
    const boundary = this.#require(options, 'boundary');
    const url = this.#require(options, 'url');
    const timeoutMs = this.#integer(options['timeout-ms'] ?? '120000', 'timeout-ms');
    const intervalMs = this.#integer(options['interval-ms'] ?? '1000', 'interval-ms');
    const recorder = new ComposeHarnessRecorder(process.env.BPANE_CI_HARNESS_EVENTS);
    const controller = new AbortController();
    const cancel = () => controller.abort();
    process.once('SIGINT', cancel);
    process.once('SIGTERM', cancel);
    try {
      const waiter = new ReadinessWaiter({
        observe: () => this.#fetchJson(url),
        record: (event) => recorder.record({
          ...event,
          namespace: process.env.BPANE_CI_STAGE_NAMESPACE ?? 'bpane-local-readiness',
        }),
      });
      await waiter.waitFor(boundary, {
        timeoutMs,
        intervalMs,
        signal: controller.signal,
        isReady: options['expect-status']
          ? (_candidate, state) => state?.status === options['expect-status']
          : boundaryReady,
      });
      console.log(`Compose ${boundary} readiness established.`);
      return 0;
    } finally {
      process.removeListener('SIGINT', cancel);
      process.removeListener('SIGTERM', cancel);
    }
  }

  async #fetchJson(url) {
    const response = await fetch(url, { signal: AbortSignal.timeout(5_000) });
    const body = await response.json().catch(() => ({}));
    return { ...body, http_status: response.status };
  }

  #parse(argv) {
    const result = {};
    for (let index = 0; index < argv.length; index += 2) {
      const name = argv[index];
      const value = argv[index + 1];
      if (!name?.startsWith('--') || value === undefined) {
        throw new Error('Compose harness options must be name/value pairs.');
      }
      result[name.slice(2)] = value;
    }
    return result;
  }

  #require(options, name) {
    const value = options[name];
    if (typeof value !== 'string' || value.length === 0) throw new Error(`missing --${name}`);
    return value;
  }

  #integer(value, name) {
    const parsed = Number.parseInt(value, 10);
    if (!Number.isInteger(parsed) || parsed <= 0) throw new Error(`--${name} must be positive.`);
    return parsed;
  }
}

try {
  process.exitCode = await new ComposeHarnessCommand().run(process.argv.slice(2));
} catch (error) {
  console.error(`Compose harness failed: ${error.code ?? 'harness_failure'}.`);
  process.exitCode = error.code === 'cancelled' ? 130 : 1;
}
