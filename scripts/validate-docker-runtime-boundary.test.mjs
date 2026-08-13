import assert from "node:assert/strict";
import test from "node:test";

import { DockerRuntimeBoundaryContract } from "./runtime-boundary/docker-runtime-boundary-contract.mjs";
import { DockerRuntimeBoundaryLiveCheck } from "./runtime-boundary/docker-runtime-boundary-live-check.mjs";

function validConfig() {
  return {
    networks: {
      "docker-control": { internal: true },
    },
    services: {
      gateway: {
        environment: { DOCKER_HOST: "tcp://docker-proxy:2375" },
        depends_on: { "docker-proxy": { condition: "service_healthy" } },
        networks: { "bpane-internal": null, "docker-control": null },
        volumes: [],
      },
      "docker-proxy": {
        image:
          "ghcr.io/tecnativa/docker-socket-proxy@sha256:" + "a".repeat(64),
        environment: {
          ALLOW_PAUSE: "0",
          ALLOW_RESTARTS: "0",
          ALLOW_START: "1",
          ALLOW_STOP: "1",
          ALLOW_UNPAUSE: "0",
          AUTH: "0",
          BUILD: "0",
          COMMIT: "0",
          CONFIGS: "0",
          CONTAINERS: "1",
          DISTRIBUTION: "0",
          EVENTS: "0",
          EXEC: "0",
          GRPC: "0",
          IMAGES: "0",
          INFO: "1",
          NETWORKS: "0",
          NODES: "0",
          PING: "1",
          PLUGINS: "0",
          POST: "1",
          SECRETS: "0",
          SERVICES: "0",
          SESSION: "0",
          SWARM: "0",
          SYSTEM: "0",
          TASKS: "0",
          VERSION: "1",
          VOLUMES: "1",
        },
        networks: { "docker-control": null },
        volumes: [
          {
            source: "/var/run/docker.sock",
            target: "/var/run/docker.sock",
            read_only: true,
          },
        ],
        cap_drop: ["ALL"],
        security_opt: ["no-new-privileges:true"],
      },
    },
  };
}

function mutationFails(mutator, expectedMessage) {
  const config = validConfig();
  mutator(config);
  assert.throws(
    () => new DockerRuntimeBoundaryContract().validate(config),
    new RegExp(expectedMessage),
  );
}

test("accepts the constrained runtime boundary", () => {
  assert.doesNotThrow(() => new DockerRuntimeBoundaryContract().validate(validConfig()));
});

test("rejects a direct gateway Docker socket mount", () => {
  mutationFails(
    (config) => {
      config.services.gateway.volumes.push({
        source: "/var/run/docker.sock",
        target: "/var/run/docker.sock",
      });
    },
    "exactly one Docker socket mount",
  );
});

test("rejects a published proxy port", () => {
  mutationFails(
    (config) => {
      config.services["docker-proxy"].ports = [{ target: 2375, published: "2375" }];
    },
    "must not publish host ports",
  );
});

test("rejects a broadened Docker API family", () => {
  mutationFails(
    (config) => {
      config.services["docker-proxy"].environment.IMAGES = "1";
    },
    "IMAGES must be 0",
  );
});

test("rejects an extra runtime-control network member", () => {
  mutationFails(
    (config) => {
      config.services.web = { networks: { "docker-control": null } };
    },
    "must contain only docker-proxy and gateway",
  );
});

test("rejects an unpinned proxy image", () => {
  mutationFails(
    (config) => {
      config.services["docker-proxy"].image = "ghcr.io/tecnativa/docker-socket-proxy:v0.5.0";
    },
    "immutable GHCR sha256 digest",
  );
});

test("rejects a writable Docker socket mount", () => {
  mutationFails(
    (config) => {
      config.services["docker-proxy"].volumes[0].read_only = false;
    },
    "Docker socket mount must be read-only",
  );
});

test("accepts the expected live API statuses", () => {
  const requests = [];
  const check = new DockerRuntimeBoundaryLiveCheck((method, endpoint) => {
    requests.push([method, endpoint]);
    return endpoint === "/_ping" ||
      endpoint === "/version" ||
      endpoint === "/info" ||
      endpoint.startsWith("/containers") ||
      endpoint === "/volumes"
      ? "200"
      : "403";
  });

  assert.doesNotThrow(() => check.validate());
  assert.ok(requests.length > 20);
});

test("rejects a live API family that becomes reachable", () => {
  const check = new DockerRuntimeBoundaryLiveCheck((_method, endpoint) =>
    endpoint === "/images/json" ? "200" : endpoint === "/_ping" ||
        endpoint === "/version" ||
        endpoint === "/info" ||
        endpoint.startsWith("/containers") ||
        endpoint === "/volumes"
      ? "200"
      : "403",
  );

  assert.throws(() => check.validate(), /denied GET \/images\/json must return 403/);
});
