import assert from "node:assert/strict";
import test from "node:test";

import { RuntimeBrokerFoundationContract } from "./runtime-broker/runtime-broker-foundation-contract.mjs";

function validConfig() {
  return {
    networks: {
      "runtime-broker-api": { internal: true },
      "runtime-broker-auth": { internal: true },
    },
    services: {
      gateway: { networks: { "runtime-broker-api": null } },
      keycloak: { networks: { "runtime-broker-auth": null } },
      "runtime-broker": {
        command:
          "--oidc-audience bpane-runtime-broker " +
          "--allowed-client-id bpane-runtime-broker-gateway " +
          "--oidc-jwks-url http://keycloak:8080/realms/browserpane-dev/certs",
        networks: {
          "runtime-broker-api": null,
          "runtime-broker-auth": null,
        },
        ports: [],
        volumes: [],
        read_only: true,
        cap_drop: ["ALL"],
        security_opt: ["no-new-privileges:true"],
        tmpfs: ["/tmp:size=16m,mode=1777"],
        healthcheck: { test: ["CMD", "curl", "http://localhost:8940/readyz"] },
      },
    },
  };
}

function mutationFails(mutator, expected) {
  const config = validConfig();
  mutator(config);
  assert.throws(
    () => new RuntimeBrokerFoundationContract().validate(config),
    new RegExp(expected),
  );
}

test("accepts the isolated broker foundation topology", () => {
  assert.doesNotThrow(() => new RuntimeBrokerFoundationContract().validate(validConfig()));
});

test("rejects public ports, docker access, and weakened confinement", () => {
  const cases = [
    [(broker) => (broker.ports = [{ published: 8940 }]), "must not publish"],
    [
      (broker) => broker.volumes.push({ source: "/var/run/docker.sock" }),
      "must not mount",
    ],
    [
      (broker) => (broker.networks["docker-control"] = null),
      "must join only",
    ],
    [(broker) => (broker.read_only = false), "must be read-only"],
    [(broker) => (broker.cap_drop = []), "drop all"],
    [(broker) => (broker.security_opt = []), "no-new-privileges"],
    [(broker) => (broker.tmpfs = []), "bounded writable /tmp"],
    [(broker) => (broker.privileged = true), "must not run as privileged"],
    [(broker) => (broker.pid = "host"), "host PID or IPC"],
    [(broker) => (broker.devices = ["/dev/kvm"]), "must not mount host devices"],
  ];
  for (const [mutate, expected] of cases) {
    mutationFails((config) => mutate(config.services["runtime-broker"]), expected);
  }
});

test("rejects extra API or auth network members", () => {
  mutationFails(
    (config) => {
      config.services.web = { networks: { "runtime-broker-api": null } };
    },
    "only gateway and runtime-broker",
  );
  mutationFails(
    (config) => {
      config.services.postgres = { networks: { "runtime-broker-auth": null } };
    },
    "only keycloak and runtime-broker",
  );
});

test("rejects missing identity constraints and command secrets", () => {
  mutationFails(
    (config) => {
      config.services["runtime-broker"].command = "--oidc-audience wrong";
    },
    "must contain --oidc-audience",
  );
  mutationFails(
    (config) => {
      config.services["runtime-broker"].command += " --client-secret leaked";
    },
    "must not contain a client secret",
  );
});
