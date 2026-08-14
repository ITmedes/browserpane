import assert from "node:assert/strict";
import test from "node:test";

import { RuntimeTracingFixtureContract } from "./runtime-tracing-fixture-contract.mjs";

function validConfig() {
  const telemetryEnvironment = {
    OTEL_TRACES_EXPORTER: "otlp",
    OTEL_EXPORTER_OTLP_ENDPOINT: "http://otel-collector:4317",
    OTEL_EXPORTER_OTLP_PROTOCOL: "grpc",
    OTEL_TRACES_SAMPLER: "always_on",
  };
  return {
    services: {
      gateway: {
        environment: telemetryEnvironment,
        depends_on: { "otel-collector": { condition: "service_started" } },
        networks: { "broker-api": null, runtime: null },
      },
      "runtime-broker": {
        environment: telemetryEnvironment,
        depends_on: { "otel-collector": { condition: "service_started" } },
        networks: { "broker-api": null, "docker-control": null },
      },
      "otel-collector": {
        image: "otel/opentelemetry-collector-contrib@sha256:e0f565815b2b8e78eb9fdbb80f6190c921ab323aaa8ccceab3255c6d4225f4af",
        read_only: true,
        cap_drop: ["ALL"],
        security_opt: ["no-new-privileges:true"],
        networks: { "broker-api": null },
        volumes: [{
          type: "bind",
          target: "/etc/otelcol-contrib/config.yaml",
          read_only: true,
        }, {
          type: "bind",
          target: "/var/lib/otel",
          read_only: false,
        }],
      },
      web: { environment: {}, networks: { runtime: null } },
    },
    networks: { "broker-api": { internal: true } },
  };
}

test("accepts the private digest-pinned tracing fixture", () => {
  assert.doesNotThrow(() => new RuntimeTracingFixtureContract().validate(validConfig()));
});

test("rejects collector exposure, mutable images, and telemetry spread", () => {
  const cases = [
    [(config) => { config.services["otel-collector"].ports = [{ target: 4317 }]; }, /must not publish/u],
    [(config) => { config.services["otel-collector"].image = "otel/opentelemetry-collector-contrib:latest"; }, /digest pinned/u],
    [(config) => { config.services["otel-collector"].networks.runtime = null; }, /networks are invalid/u],
    [(config) => { config.services.gateway.environment.OTEL_EXPORTER_OTLP_ENDPOINT = "https://token@collector.example"; }, /private collector endpoint/u],
    [(config) => { config.services.web.environment.OTEL_TRACES_EXPORTER = "otlp"; }, /must not receive/u],
  ];
  for (const [mutate, expected] of cases) {
    const config = validConfig();
    mutate(config);
    assert.throws(() => new RuntimeTracingFixtureContract().validate(config), expected);
  }
});
