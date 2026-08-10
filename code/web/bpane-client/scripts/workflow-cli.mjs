#!/usr/bin/env node

import process from 'node:process';
import { pathToFileURL } from 'node:url';

import { runBpaneCli } from './bpane-cli.mjs';

export async function runWorkflowCli(
  argv,
  env = process.env,
  io = process,
  fetchImpl = globalThis.fetch,
) {
  return await runBpaneCli(argv, env, io, fetchImpl);
}

const mainUrl = process.argv[1] ? pathToFileURL(process.argv[1]).href : '';
if (import.meta.url === mainUrl) {
  process.exitCode = await runWorkflowCli(process.argv.slice(2));
}
