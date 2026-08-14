import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import { SingleNodeConfigRenderer } from "../../deploy/single-node/render-config.mjs";
import { SingleNodeComposeContract } from "./single-node-compose-contract.mjs";
import {
  SINGLE_NODE_SECRET_FILE_INPUTS,
  SingleNodeInputContract,
} from "./single-node-input-contract.mjs";

export class SingleNodePreflight {
  constructor(rootDirectory) {
    this.rootDirectory = rootDirectory;
  }

  run(environment) {
    new SingleNodeInputContract().validate(environment);
    new SingleNodeConfigRenderer().render(
      environment,
      path.join(this.rootDirectory, "deploy/single-node/generated"),
    );
    const composeConfig = this.composeConfig(environment);
    const input = this.contractInput(composeConfig);
    new SingleNodeComposeContract().validate(input);
    this.rejectSecretLeakage(environment, input);
    return input;
  }

  composeConfig(environment) {
    const result = spawnSync(
      "docker",
      ["compose", "-f", "deploy/single-node/compose.yml", "config", "--format", "json"],
      {
        cwd: this.rootDirectory,
        encoding: "utf8",
        env: { ...process.env, ...environment },
        maxBuffer: 16 * 1024 * 1024,
      },
    );
    if (result.error || result.status !== 0) {
      throw new Error(`single-node Compose render failed: ${result.error?.message ?? result.stderr.trim()}`);
    }
    return JSON.parse(result.stdout);
  }

  contractInput(composeConfig) {
    return {
      composeConfig,
      composeSource: this.read("deploy/single-node/compose.yml"),
      webDockerfile: this.read("deploy/Dockerfile.web-production"),
      nginxConfig: this.read("deploy/single-node/nginx.conf"),
      authTemplate: this.read("deploy/single-node/auth-config.template.json"),
    };
  }

  rejectSecretLeakage(environment, input) {
    const rendered = JSON.stringify(input.composeConfig);
    for (const name of SINGLE_NODE_SECRET_FILE_INPUTS) {
      const value = fs.readFileSync(environment[name], "utf8").trim();
      if (value && rendered.includes(value)) throw new Error(`${name} content leaked into Compose`);
    }
  }

  read(relativePath) {
    return fs.readFileSync(path.join(this.rootDirectory, relativePath), "utf8");
  }
}

export class SingleNodeRepositoryFixture {
  createEnvironment() {
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), "bpane-single-node-preflight-"));
    const files = {};
    for (const [name, value] of Object.entries({
      BPANE_DATABASE_URL_FILE: "fixture-postgres-url",
      BPANE_VAULT_TOKEN_FILE: "fixture-vault-token",
      BPANE_BROKER_GATEWAY_CLIENT_SECRET_FILE: "fixture-broker-secret",
      BPANE_WORKER_OIDC_CLIENT_SECRET_FILE: "fixture-worker-secret",
      BPANE_TLS_KEY_FILE: "fixture-private-key",
      BPANE_TLS_CERT_FILE: "fixture-public-certificate",
    })) {
      const filename = path.join(directory, name.toLowerCase());
      fs.writeFileSync(filename, `${value}\n`, { mode: 0o600 });
      files[name] = filename;
    }
    const digest = (character) => `sha256:${character.repeat(64)}`;
    return {
      ...files,
      BPANE_DEPLOYMENT_NAME: "bpane-preflight",
      BPANE_WEB_IMAGE: digest("a"),
      BPANE_GATEWAY_IMAGE: digest("b"),
      BPANE_RUNTIME_BROKER_IMAGE: digest("c"),
      BPANE_DOCKER_PROXY_IMAGE: digest("d"),
      BPANE_BROWSER_IMAGE: digest("e"),
      BPANE_WORKFLOW_WORKER_IMAGE: digest("f"),
      BPANE_RECORDING_WORKER_IMAGE: digest("1"),
      BPANE_STORAGE_HELPER_IMAGE: digest("e"),
      BPANE_WEB_BIND_ADDRESS: "127.0.0.1",
      BPANE_PUBLIC_GATEWAY_URL: "https://browser.example:4433",
      BPANE_BROWSER_START_URL: "https://example.org/",
      BPANE_OIDC_PUBLIC_ISSUER: "https://identity.example/realms/bpane",
      BPANE_OIDC_INTERNAL_JWKS_URL: "https://identity.example/jwks",
      BPANE_OIDC_TOKEN_URL: "https://identity.example/token",
      BPANE_OIDC_GATEWAY_AUDIENCE: "bpane-gateway",
      BPANE_OIDC_BROWSER_CLIENT_ID: "bpane-web",
      BPANE_OIDC_BROKER_AUDIENCE: "bpane-runtime-broker",
      BPANE_OIDC_BROKER_GATEWAY_CLIENT_ID: "bpane-runtime-broker-gateway",
      BPANE_OIDC_WORKER_CLIENT_ID: "bpane-worker",
      BPANE_VAULT_ADDR: "https://vault.example",
    };
  }
}
