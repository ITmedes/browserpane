import assert from "node:assert/strict";
import test from "node:test";

import { RuntimeBrokerBrowserOverlayContract } from "./runtime-broker/runtime-broker-browser-overlay-contract.mjs";

function validConfig() {
  const digest = `sha256:${"a".repeat(64)}`;
  return {
    services: {
      "docker-proxy": { networks: { "docker-control": null } },
      keycloak: { networks: { "runtime-broker-auth": null } },
      gateway: {
        command: ["--runtime-backend", "broker_pool"],
        environment: {
          BPANE_GATEWAY_RUNTIME_BROKER_URL: "http://runtime-broker:8940",
          BPANE_GATEWAY_RUNTIME_BROKER_TOKEN_URL:
            "http://keycloak:8080/realms/browserpane-dev/protocol/openid-connect/token",
          BPANE_GATEWAY_RUNTIME_BROKER_CLIENT_ID: "bpane-runtime-broker-gateway",
        },
        volumes: [{ target: "/run/secrets/runtime-broker-client-secret", read_only: true }],
        depends_on: { "runtime-broker": { condition: "service_healthy" } },
        networks: { "docker-control": null, "runtime-broker-api": null },
      },
      "runtime-broker": {
        environment: {
          BPANE_RUNTIME_BROKER_EXECUTOR: "docker-browser",
          BPANE_RUNTIME_BROKER_DOCKER_API_URL: "http://docker-proxy:2375",
          BPANE_RUNTIME_BROKER_BROWSER_IMAGE: digest,
        },
        volumes: [
          { target: "/runtime-config/extensions.json", read_only: true },
          { target: "/runtime-config/host-runtime.env", read_only: true },
        ],
        depends_on: { "docker-proxy": { condition: "service_healthy" } },
        networks: {
          "docker-control": null,
          "runtime-broker-api": null,
          "runtime-broker-auth": null,
        },
      },
    },
  };
}

function mutationFails(mutate, expected) {
  const config = validConfig();
  mutate(config);
  assert.throws(
    () => new RuntimeBrokerBrowserOverlayContract().validate(config),
    new RegExp(expected),
  );
}

test("accepts the opt-in browser broker topology", () => {
  new RuntimeBrokerBrowserOverlayContract().validate(validConfig());
});

test("rejects mutable images, direct sockets, and writable configuration", () => {
  mutationFails(
    (config) => {
      config.services["runtime-broker"].environment.BPANE_RUNTIME_BROKER_BROWSER_IMAGE =
        "deploy-host:latest";
    },
    "image must be immutable",
  );
  mutationFails(
    (config) => config.services["runtime-broker"].volumes.push({ source: "/var/run/docker.sock" }),
    "only through the proxy",
  );
  mutationFails(
    (config) => {
      config.services["runtime-broker"].volumes[0].read_only = false;
    },
    "must be mounted read-only",
  );
});

test("rejects bypassed broker routing and missing startup dependencies", () => {
  mutationFails(
    (config) => {
      config.services.gateway.command = ["--runtime-backend", "docker_pool"];
    },
    "select broker_pool",
  );
  mutationFails(
    (config) => {
      delete config.services.gateway.depends_on["runtime-broker"];
    },
    "wait for a healthy runtime-broker",
  );
  mutationFails(
    (config) => {
      config.services.web = { networks: { "docker-control": null } };
    },
    "docker-control must contain only",
  );
});
