#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const DEPLOYMENT_NAME_PATTERN = /^[a-z0-9][a-z0-9-]{0,47}$/u;

export class SingleNodeConfigRenderer {
  render(environment, outputDirectory) {
    const deploymentName = this.required(environment, 'BPANE_DEPLOYMENT_NAME');
    if (!DEPLOYMENT_NAME_PATTERN.test(deploymentName)) {
      throw new Error('BPANE_DEPLOYMENT_NAME must match [a-z0-9][a-z0-9-]{0,47}');
    }

    const publicGatewayUrl = this.url(environment, 'BPANE_PUBLIC_GATEWAY_URL', ['https:']);
    const oidcTokenUrl = this.url(environment, 'BPANE_OIDC_TOKEN_URL', ['https:', 'http:']);
    const workerClientId = this.required(environment, 'BPANE_OIDC_WORKER_CLIENT_ID');
    const browserStartUrl = this.url(environment, 'BPANE_BROWSER_START_URL', ['https:', 'http:']);
    const runtimeNetwork = `${deploymentName}-runtime`;

    const browserEnvironment = [
      'BPANE_H264_MODE=video_tiles',
      'BPANE_H264_BITRATE=12M',
      'BPANE_H264_MAXRATE=16M',
      'BPANE_H264_BUFSIZE=6M',
      'BPANE_H264_PRESET=ultrafast',
      'BPANE_H264_PROFILE=baseline',
      'BPANE_H264_LEVEL=4.2',
      'BPANE_H264_TUNE=zerolatency',
      'BPANE_H264_BFRAMES=0',
      `BPANE_FPS=${this.integer(environment, 'BPANE_BROWSER_FPS', 30, 1, 60)}`,
      'BPANE_AUDIO_CODEC=opus',
      'BPANE_TILE_CODEC=zstd',
      `BPANE_URL=${browserStartUrl.href}`,
      'BPANE_DPI=150',
      'GDK_SCALE=2',
      'GDK_DPI_SCALE=0.8',
      'BPANE_PROFILE_ROOT=/run/bpane/profiles',
      'RUST_LOG=info,bpane_host=info',
      '',
    ].join('\n');

    const workers = {
      version: 1,
      oidc: {
        token_url: oidcTokenUrl.href,
        client_id: workerClientId,
        scopes: environment.BPANE_OIDC_WORKER_SCOPES?.trim() ?? '',
      },
      workflow: {
        network: runtimeNetwork,
        container_name_prefix: `${deploymentName}-workflow`,
        gateway_api_url: 'http://gateway:8932',
        work_root: '/tmp/bpane-workflows',
        request_timeout_ms: this.integer(
          environment,
          'BPANE_WORKER_REQUEST_TIMEOUT_MS',
          30000,
          1000,
          300000,
        ),
        output_limit_bytes: this.integer(
          environment,
          'BPANE_WORKER_OUTPUT_LIMIT_BYTES',
          262144,
          1024,
          10485760,
        ),
      },
      recording: {
        network: runtimeNetwork,
        container_name_prefix: `${deploymentName}-recording`,
        artifact_volume: `${deploymentName}-recording-staging`,
        chrome_executable: '/usr/bin/chromium',
        gateway_api_url: 'http://gateway:8932',
        page_url: 'http://web:8080/recording-worker.html',
        connect_gateway_url: publicGatewayUrl.href.replace(/\/$/u, ''),
        output_root: '/var/lib/browserpane/recording-staging',
        cert_spki: null,
        cert_spki_file: null,
        headless: true,
        connect_timeout_ms: this.integer(
          environment,
          'BPANE_RECORDING_CONNECT_TIMEOUT_MS',
          120000,
          1000,
          300000,
        ),
        poll_interval_ms: 2000,
        request_timeout_ms: this.integer(
          environment,
          'BPANE_WORKER_REQUEST_TIMEOUT_MS',
          30000,
          1000,
          300000,
        ),
      },
    };

    fs.mkdirSync(outputDirectory, { recursive: true, mode: 0o750 });
    const browserEnvironmentFile = path.join(outputDirectory, 'browser.env');
    const workerPolicyFile = path.join(outputDirectory, 'workers.json');
    fs.writeFileSync(browserEnvironmentFile, browserEnvironment, { mode: 0o644 });
    fs.writeFileSync(
      workerPolicyFile,
      `${JSON.stringify(workers, null, 2)}\n`,
      { mode: 0o644 },
    );
    fs.chmodSync(browserEnvironmentFile, 0o644);
    fs.chmodSync(workerPolicyFile, 0o644);
  }

  required(environment, name) {
    const value = environment[name]?.trim();
    if (!value) throw new Error(`${name} is required`);
    if (value.includes('\n') || value.includes('\r')) throw new Error(`${name} must be one line`);
    return value;
  }

  url(environment, name, protocols) {
    const value = this.required(environment, name);
    let parsed;
    try {
      parsed = new URL(value);
    } catch {
      throw new Error(`${name} must be an absolute URL`);
    }
    if (!protocols.includes(parsed.protocol)) {
      throw new Error(`${name} must use ${protocols.join(' or ')}`);
    }
    if (parsed.username || parsed.password) throw new Error(`${name} must not contain credentials`);
    return parsed;
  }

  integer(environment, name, fallback, minimum, maximum) {
    const source = environment[name]?.trim();
    if (!source) return fallback;
    const value = Number(source);
    if (!Number.isSafeInteger(value) || value < minimum || value > maximum) {
      throw new Error(`${name} must be an integer between ${minimum} and ${maximum}`);
    }
    return value;
  }
}

function parseOutputDirectory(args) {
  if (args.length === 0) return path.resolve('deploy/single-node/generated');
  if (args.length === 2 && args[0] === '--output') return path.resolve(args[1]);
  throw new Error('usage: render-config.mjs [--output DIRECTORY]');
}

const invokedPath = process.argv[1] ? path.resolve(process.argv[1]) : null;
if (invokedPath === fileURLToPath(import.meta.url)) {
  try {
    new SingleNodeConfigRenderer().render(process.env, parseOutputDirectory(process.argv.slice(2)));
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
