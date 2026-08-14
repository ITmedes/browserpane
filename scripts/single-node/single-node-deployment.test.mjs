import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { before, test } from "node:test";

import { EnvironmentFileParser } from "./env-file-parser.mjs";
import { SingleNodeComposeContract } from "./single-node-compose-contract.mjs";
import { SingleNodeInputContract } from "./single-node-input-contract.mjs";
import { SingleNodePreflight, SingleNodeRepositoryFixture } from "./single-node-preflight.mjs";

const rootDirectory = path.resolve(import.meta.dirname, "../..");
let validEnvironment;
let validContractInput;

before(() => {
  validEnvironment = new SingleNodeRepositoryFixture().createEnvironment();
  validContractInput = new SingleNodePreflight(rootDirectory).run(validEnvironment);
});

test("repository fixture passes the complete single-node preflight", () => {
  assert.equal(validContractInput.composeConfig.services.gateway.read_only, true);
  assert.deepEqual(
    Object.keys(validContractInput.composeConfig.services).sort(),
    ["docker-proxy", "gateway", "runtime-broker", "web"],
  );
});

test("input contract rejects unsafe deployment values", () => {
  const contract = new SingleNodeInputContract();
  const cases = [
    [{ BPANE_GATEWAY_IMAGE: "browserpane/gateway:latest" }, /must be immutable/u],
    [{ BPANE_PUBLIC_GATEWAY_URL: "http://browser.example" }, /must use https/u],
    [{ BPANE_PUBLIC_GATEWAY_URL: "https://localhost:4433" }, /must not use a local host/u],
    [{ BPANE_WEB_BIND_ADDRESS: "0.0.0.0" }, /must be loopback/u],
    [{ BPANE_MAX_ACTIVE_RUNTIMES: "2", BPANE_MAX_STARTING_RUNTIMES: "3" },
      /must not exceed active/u],
    [{ BPANE_DATABASE_URL: "postgres://inline" }, /inline secret input is forbidden/u],
    [{ BPANE_STORAGE_HELPER_IMAGE: `sha256:${"9".repeat(64)}` }, /pinned browser image/u],
  ];
  for (const [override, expected] of cases) {
    assert.throws(() => contract.validate({ ...validEnvironment, ...override }), expected);
  }
});

test("input contract rejects broad secret permissions and symlinks", () => {
  if (process.platform === "win32") return;
  const broad = new SingleNodeRepositoryFixture().createEnvironment();
  fs.chmodSync(broad.BPANE_VAULT_TOKEN_FILE, 0o640);
  assert.throws(() => new SingleNodeInputContract().validate(broad), /must be owner-only/u);

  const linked = new SingleNodeRepositoryFixture().createEnvironment();
  const target = linked.BPANE_DATABASE_URL_FILE;
  const symlink = `${target}-link`;
  fs.symlinkSync(target, symlink);
  linked.BPANE_DATABASE_URL_FILE = symlink;
  assert.throws(() => new SingleNodeInputContract().validate(linked), /must not reference a symlink/u);
});

test("compose contract rejects topology, listener, secret, and fixture regressions", () => {
  const contract = new SingleNodeComposeContract();
  const cases = [
    [(input) => { input.composeConfig.services.gateway.image = "gateway:latest"; }, /image must be immutable/u],
    [(input) => { input.composeConfig.services.web.ports[0].host_ip = "0.0.0.0"; }, /bind loopback/u],
    [(input) => { input.composeConfig.services["runtime-broker"].ports = [{ target: 8940 }]; },
      /must not publish host ports/u],
    [(input) => { input.composeConfig.services.gateway.networks["docker-control"] = null; },
      /gateway networks are invalid/u],
    [(input) => { input.composeConfig.services.gateway.command.push("--database-url", "secret"); },
      /inline configuration is forbidden/u],
    [(input) => {
      input.composeConfig.services.gateway.volumes.find(
        (mount) => mount.target === "/var/lib/browserpane/recording-artifacts",
      ).read_only = true;
    }, /read-only policy is invalid/u],
    [(input) => { input.composeConfig.services["docker-proxy"].environment.EXEC = "1"; },
      /docker-proxy EXEC must be denied/u],
    [(input) => {
      input.composeConfig.services["runtime-broker"]
        .environment.BPANE_RUNTIME_BROKER_WORKFLOW_IMAGE = "workflow:latest";
    }, /BPANE_RUNTIME_BROKER_WORKFLOW_IMAGE must be immutable/u],
    [(input) => {
      input.composeConfig.services.gateway.volumes = input.composeConfig.services.gateway.volumes
        .filter((mount) => mount.target !== "/run/secrets/vault-token");
    }, /vault-token must use a bind mount/u],
    [(input) => { input.composeSource += "\n# demo-demo\n"; }, /contains demo-demo/u],
    [(input) => { input.webDockerfile += "\nCOPY dev/web-fixtures/test.html /tmp\n"; },
      /contains dev\/web-fixtures/u],
  ];
  for (const [mutate, expected] of cases) {
    const input = structuredClone(validContractInput);
    mutate(input);
    assert.throws(() => contract.validate(input), expected);
  }
});

test("preflight detects secret content copied into rendered Compose", () => {
  const environment = new SingleNodeRepositoryFixture().createEnvironment();
  fs.writeFileSync(environment.BPANE_DATABASE_URL_FILE, `${environment.BPANE_WEB_IMAGE}\n`, {
    mode: 0o600,
  });
  assert.throws(
    () => new SingleNodePreflight(rootDirectory).run(environment),
    /BPANE_DATABASE_URL_FILE content leaked into Compose/u,
  );
});

test("environment file parser handles comments and rejects duplicates", () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "bpane-env-parser-"));
  const valid = path.join(directory, "valid.env");
  fs.writeFileSync(valid, "# deployment\nBPANE_NAME='browserpane'\nBPANE_PORT=8080\n");
  assert.deepEqual(new EnvironmentFileParser().parse(valid), {
    BPANE_NAME: "browserpane",
    BPANE_PORT: "8080",
  });
  const duplicate = path.join(directory, "duplicate.env");
  fs.writeFileSync(duplicate, "BPANE_NAME=one\nBPANE_NAME=two\n");
  assert.throws(() => new EnvironmentFileParser().parse(duplicate), /duplicate environment entry/u);
});
