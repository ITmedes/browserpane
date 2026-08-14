import fs from "node:fs";
import net from "node:net";

const IMAGE_INPUTS = [
  "BPANE_WEB_IMAGE",
  "BPANE_GATEWAY_IMAGE",
  "BPANE_RUNTIME_BROKER_IMAGE",
  "BPANE_DOCKER_PROXY_IMAGE",
  "BPANE_BROWSER_IMAGE",
  "BPANE_WORKFLOW_WORKER_IMAGE",
  "BPANE_RECORDING_WORKER_IMAGE",
  "BPANE_STORAGE_HELPER_IMAGE",
];

const OWNER_ONLY_FILES = [
  "BPANE_DATABASE_URL_FILE",
  "BPANE_VAULT_TOKEN_FILE",
  "BPANE_BROKER_GATEWAY_CLIENT_SECRET_FILE",
  "BPANE_WORKER_OIDC_CLIENT_SECRET_FILE",
  "BPANE_TLS_KEY_FILE",
];

const REQUIRED_TEXT = [
  "BPANE_DEPLOYMENT_NAME",
  "BPANE_OIDC_BROWSER_CLIENT_ID",
  "BPANE_OIDC_GATEWAY_AUDIENCE",
  "BPANE_OIDC_BROKER_AUDIENCE",
  "BPANE_OIDC_BROKER_GATEWAY_CLIENT_ID",
  "BPANE_OIDC_WORKER_CLIENT_ID",
];

export class SingleNodeInputContract {
  validate(environment) {
    for (const name of REQUIRED_TEXT) this.required(environment, name);
    if (!/^[a-z0-9][a-z0-9-]{0,21}$/u.test(environment.BPANE_DEPLOYMENT_NAME)) {
      throw new Error(
        "BPANE_DEPLOYMENT_NAME is invalid or too long for runtime DNS labels",
      );
    }
    for (const name of IMAGE_INPUTS) this.immutableImage(environment, name);
    this.expect(
      environment.BPANE_STORAGE_HELPER_IMAGE === environment.BPANE_BROWSER_IMAGE,
      "BPANE_STORAGE_HELPER_IMAGE must use the pinned browser image",
    );
    for (const name of [
      "BPANE_PUBLIC_GATEWAY_URL",
      "BPANE_OIDC_PUBLIC_ISSUER",
      "BPANE_OIDC_INTERNAL_JWKS_URL",
      "BPANE_OIDC_TOKEN_URL",
      "BPANE_VAULT_ADDR",
    ]) {
      this.secureUrl(environment, name);
    }
    this.httpsUrl(environment, "BPANE_RECORDING_CONNECT_GATEWAY_URL");
    this.httpUrl(environment, "BPANE_BROWSER_START_URL");
    this.loopback(environment.BPANE_WEB_BIND_ADDRESS ?? "127.0.0.1");
    for (const name of ["BPANE_WEB_PORT", "BPANE_WEBTRANSPORT_PORT"]) {
      if (environment[name] !== undefined) this.integer(environment, name, 1, 65535);
    }
    const active = this.integer(environment, "BPANE_MAX_ACTIVE_RUNTIMES", 1, 1024, 4);
    const starting = this.integer(environment, "BPANE_MAX_STARTING_RUNTIMES", 1, 1024, 1);
    this.expect(starting <= active, "BPANE_MAX_STARTING_RUNTIMES must not exceed active runtimes");
    this.integer(environment, "BPANE_MAX_VIEWERS", 1, 1000, 10);
    this.integer(environment, "BPANE_BROKER_MAX_CONCURRENT", 1, 1024, 16);
    this.integer(environment, "BPANE_WORKFLOW_MAX_ACTIVE", 1, 1024, 4);
    for (const name of [
      "BPANE_DATABASE_URL",
      "BPANE_VAULT_TOKEN",
      "BPANE_BROKER_GATEWAY_CLIENT_SECRET",
      "BPANE_WORKER_OIDC_CLIENT_SECRET",
    ]) {
      this.expect(environment[name] === undefined, `${name} inline secret input is forbidden`);
    }
    for (const name of OWNER_ONLY_FILES) this.file(environment, name, true);
    this.file(environment, "BPANE_TLS_CERT_FILE", false);
  }

  required(environment, name) {
    const value = environment[name]?.trim();
    this.expect(value && !/[\r\n]/u.test(value), `${name} is required and must be one line`);
    return value;
  }

  immutableImage(environment, name) {
    const value = this.required(environment, name);
    const digest = value.startsWith("sha256:") ? value.slice(7) : value.split("@sha256:")[1];
    this.expect(/^[a-fA-F0-9]{64}$/u.test(digest ?? ""), `${name} must be immutable`);
  }

  secureUrl(environment, name) {
    const parsed = this.url(environment, name);
    this.expect(parsed.protocol === "https:", `${name} must use https`);
    this.expect(!this.isLocalHost(parsed.hostname), `${name} must not use a local host`);
  }

  httpUrl(environment, name) {
    const parsed = this.url(environment, name);
    this.expect(["https:", "http:"].includes(parsed.protocol), `${name} must use http or https`);
  }

  httpsUrl(environment, name) {
    const parsed = this.url(environment, name);
    this.expect(parsed.protocol === "https:", `${name} must use https`);
  }

  url(environment, name) {
    let parsed;
    try {
      parsed = new URL(this.required(environment, name));
    } catch {
      throw new Error(`${name} must be an absolute URL`);
    }
    this.expect(!parsed.username && !parsed.password, `${name} must not contain credentials`);
    return parsed;
  }

  file(environment, name, ownerOnly) {
    const filename = this.required(environment, name);
    let metadata;
    try {
      metadata = fs.lstatSync(filename);
    } catch {
      throw new Error(`${name} cannot be inspected`);
    }
    this.expect(!metadata.isSymbolicLink(), `${name} must not reference a symlink`);
    this.expect(metadata.isFile(), `${name} must reference a regular file`);
    this.expect(metadata.size > 0 && metadata.size <= 65536, `${name} has an invalid size`);
    if (ownerOnly && process.platform !== "win32") {
      this.expect((metadata.mode & 0o077) === 0, `${name} must be owner-only`);
    }
  }

  integer(environment, name, minimum, maximum, fallback) {
    const source = environment[name]?.trim();
    const value = source ? Number(source) : fallback;
    this.expect(Number.isSafeInteger(value) && value >= minimum && value <= maximum,
      `${name} must be an integer between ${minimum} and ${maximum}`);
    return value;
  }

  loopback(value) {
    this.expect(value === "localhost" || net.isIP(value) > 0, "BPANE_WEB_BIND_ADDRESS is invalid");
    this.expect(value === "localhost" || value === "127.0.0.1" || value === "::1",
      "BPANE_WEB_BIND_ADDRESS must be loopback");
  }

  isLocalHost(hostname) {
    return hostname === "localhost" || hostname === "127.0.0.1" || hostname === "::1";
  }

  expect(condition, message) {
    if (!condition) throw new Error(message);
  }
}

export const SINGLE_NODE_SECRET_FILE_INPUTS = [...OWNER_ONLY_FILES];
