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
          BPANE_RUNTIME_BROKER_WORKFLOW_IMAGE: digest,
          BPANE_RUNTIME_BROKER_RECORDING_IMAGE: digest,
          BPANE_RUNTIME_BROKER_STORAGE_HELPER_IMAGE: digest,
          BPANE_RUNTIME_BROKER_WORKER_CONFIG_FILE: "/runtime-config/workers.json",
          BPANE_RUNTIME_BROKER_WORKER_OIDC_CLIENT_SECRET_FILE:
            "/run/secrets/worker-oidc-client-secret",
        },
        volumes: [
          { target: "/runtime-config/extensions.json", read_only: true },
          { target: "/runtime-config/host-runtime.env", read_only: true },
          { target: "/runtime-config/workers.json", read_only: true },
          { target: "/run/secrets/worker-oidc-client-secret", read_only: true },
          { target: "/certs", read_only: true },
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

test("accepts the opt-in browser and worker broker topology", () => {
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

test("rejects mutable worker images and unsafe worker inputs", () => {
  mutationFails(
    (config) => {
      config.services["runtime-broker"].environment.BPANE_RUNTIME_BROKER_WORKFLOW_IMAGE =
        "deploy-workflow-worker:latest";
    },
    "workflow image must be immutable",
  );
  mutationFails(
    (config) => {
      config.services["runtime-broker"].environment.BPANE_RUNTIME_BROKER_RECORDING_IMAGE =
        "deploy-recording-worker:latest";
    },
    "recording image must be immutable",
  );
  mutationFails(
    (config) => {
      config.services["runtime-broker"].volumes[2].read_only = false;
    },
    "workers.json must be mounted read-only",
  );
  mutationFails(
    (config) => {
      const environment = config.services["runtime-broker"].environment;
      environment.BPANE_RUNTIME_BROKER_WORKER_OIDC_CLIENT_SECRET_FILE = undefined;
    },
    "file-backed worker OIDC secret",
  );
});

test("rejects mutable or divergent storage helper images", () => {
  mutationFails(
    (config) => {
      config.services["runtime-broker"].environment.BPANE_RUNTIME_BROKER_STORAGE_HELPER_IMAGE =
        "deploy-host:latest";
    },
    "storage helper image must be immutable",
  );
  mutationFails(
    (config) => {
      config.services["runtime-broker"].environment.BPANE_RUNTIME_BROKER_STORAGE_HELPER_IMAGE =
        `sha256:${"b".repeat(64)}`;
    },
    "storage helper must use the pinned host image",
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
