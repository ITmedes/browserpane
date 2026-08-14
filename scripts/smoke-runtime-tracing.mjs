#!/usr/bin/env node

import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { randomBytes } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { traceSpans } from "./runtime-tracing/otlp-trace-evidence.mjs";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const API_ORIGIN = "http://localhost:18080";
const TOKEN_URL = "http://localhost:18091/realms/browserpane-dev/protocol/openid-connect/token";
const TRACE_FILE = path.join(ROOT, "deploy/single-node/generated/telemetry/traces.jsonl");
const TIMEOUT_MS = 120_000;
const COMPOSE_ARGS = [
  "compose",
  "--project-name",
  "bpane-single-node-fixture",
  "--env-file",
  "deploy/single-node/generated/fixture.env",
  "-f",
  "deploy/single-node/compose.yml",
  "-f",
  "deploy/single-node/fixture/compose.yml",
];
const REQUIRED_SPANS = [
  "browserpane.http.server",
  "browserpane.runtime_broker.client",
  "browserpane.runtime_broker.authenticate",
  "browserpane.runtime_broker.execute",
  "browserpane.runtime.policy",
  "browserpane.runtime.docker",
];
const ALLOWED_TRACE_ATTRIBUTES = new Set([
  "browserpane.operation.action",
  "browserpane.operation.kind",
  "browserpane.result",
  "browserpane.runtime.stage",
  "http.request.method",
  "http.response.status_code",
  "http.route",
]);
const FIXTURE_SECRETS = [
  "single-node-fixture-vault-token",
  "bpane-runtime-broker-gateway-secret",
  "bpane-mcp-bridge-secret",
  "postgres://browserpane:single-node-fixture",
];

async function main() {
  assertFixtureRunning();
  const accessToken = await issueServiceToken();
  const sessionIds = [];
  const sensitiveMarkers = [...FIXTURE_SECRETS, accessToken];

  try {
    const correlated = traceContext();
    const first = await createSession(accessToken, "trace-correlation-marker");
    sessionIds.push(first.id);
    sensitiveMarkers.push(first.id, "trace-correlation-marker");
    const firstAccess = await startRuntime(accessToken, first.id, {
      traceparent: correlated.traceparent,
      tracestate: "vendor=bpane-smoke",
      baggage: "private=trace-baggage-redaction-marker",
    });
    assert.equal(firstAccess.response.headers.get("traceparent"), null,
      "gateway reflected traceparent to the public caller");
    assert.equal(firstAccess.response.headers.get("tracestate"), null,
      "gateway reflected tracestate to the public caller");
    sensitiveMarkers.push(
      firstAccess.payload.token,
      "trace-baggage-redaction-marker",
    );
    const firstTrace = await waitForTrace(correlated.traceId, hasCompleteLifecycleTrace);
    assertTraceContract(firstTrace, correlated.parentSpanId);

    const malformed = await createSession(accessToken, "malformed-context-marker");
    sessionIds.push(malformed.id);
    sensitiveMarkers.push(malformed.id, "malformed-context-marker");
    const malformedAccess = await startRuntime(accessToken, malformed.id, {
      traceparent: "malformed-trace-context-marker",
      tracestate: "vendor=malformed-state-marker",
      baggage: "private=malformed-baggage-marker",
    });
    assert(malformedAccess.payload.automation?.endpoint_url,
      "malformed caller context changed the runtime operation result");
    sensitiveMarkers.push(
      malformedAccess.payload.token,
      "malformed-trace-context-marker",
      "malformed-state-marker",
      "malformed-baggage-marker",
    );

    dockerCompose(["stop", "otel-collector"]);
    const outage = await createSession(accessToken, "collector-outage-marker");
    sessionIds.push(outage.id);
    sensitiveMarkers.push(outage.id, "collector-outage-marker");
    const outageAccess = await startRuntime(accessToken, outage.id, traceHeaders(traceContext()));
    assert(outageAccess.payload.automation?.endpoint_url,
      "collector outage changed the runtime operation result");
    sensitiveMarkers.push(outageAccess.payload.token);
    assertGatewayReady();

    dockerCompose(["start", "otel-collector"]);
    await waitForCollectorRunning();
    const recovered = traceContext();
    const recoverySession = await createSession(accessToken, "collector-recovery-marker");
    sessionIds.push(recoverySession.id);
    sensitiveMarkers.push(recoverySession.id, "collector-recovery-marker");
    const recoveryAccess = await startRuntime(
      accessToken,
      recoverySession.id,
      traceHeaders(recovered),
    );
    sensitiveMarkers.push(recoveryAccess.payload.token);
    const recoveryTrace = await waitForTrace(recovered.traceId, hasCompleteLifecycleTrace);
    assertTraceContract(recoveryTrace, recovered.parentSpanId);
    assertSensitiveValuesAbsent(sensitiveMarkers);

    console.log(JSON.stringify({
      callerContextContinued: true,
      collectorOutageTolerated: true,
      collectorRecoveryObserved: true,
      malformedContextIgnored: true,
      redactionMarkersAbsent: sensitiveMarkers.length,
      firstTraceId: correlated.traceId,
      firstTraceSpanCount: firstTrace.length,
      recoveredTraceId: recovered.traceId,
      recoveredTraceSpanCount: recoveryTrace.length,
      services: [...new Set(firstTrace.map((span) => span.serviceName))].sort(),
    }, null, 2));
  } finally {
    dockerCompose(["start", "otel-collector"], { allowFailure: true });
    for (const sessionId of sessionIds) await removeSession(accessToken, sessionId);
  }
}

function assertFixtureRunning() {
  assert(fs.existsSync(path.join(ROOT, "deploy/single-node/generated/fixture.env")),
    "single-node fixture environment is missing; run scripts/start-single-node-fixture.sh");
  const result = dockerCompose(["ps", "--status", "running", "--services"], { capture: true });
  const services = new Set(result.trim().split("\n").filter(Boolean));
  for (const service of ["gateway", "runtime-broker", "otel-collector", "web", "keycloak"]) {
    assert(services.has(service), `single-node fixture service ${service} is not running`);
  }
}

async function issueServiceToken() {
  const response = await fetch(TOKEN_URL, {
    method: "POST",
    headers: { "content-type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams({
      grant_type: "client_credentials",
      client_id: "bpane-mcp-bridge",
      client_secret: "bpane-mcp-bridge-secret",
    }),
  });
  assert(response.ok, `fixture token request failed with HTTP ${response.status}`);
  const payload = await response.json();
  assert(payload.access_token, "fixture token response is missing access_token");
  return payload.access_token;
}

async function createSession(accessToken, marker) {
  const { payload } = await api("/api/v1/sessions", accessToken, {
    method: "POST",
    body: { labels: { suite: "runtime-tracing", marker } },
  });
  assert(payload.id, "session response is missing id");
  return payload;
}

async function startRuntime(accessToken, sessionId, headers) {
  return await api(`/api/v1/sessions/${sessionId}/automation-access`, accessToken, {
    method: "POST",
    body: {},
    headers,
  });
}

async function api(resourcePath, accessToken, options = {}) {
  const headers = {
    authorization: `Bearer ${accessToken}`,
    ...(options.headers ?? {}),
  };
  const init = { method: options.method ?? "GET", headers };
  if (options.body !== undefined) {
    headers["content-type"] = "application/json";
    init.body = JSON.stringify(options.body);
  }
  const response = await fetch(`${API_ORIGIN}${resourcePath}`, init);
  const text = await response.text();
  assert(response.ok,
    `${init.method} ${resourcePath} failed with HTTP ${response.status}${text ? `: ${text}` : ""}`);
  return { response, payload: text ? JSON.parse(text) : null };
}

function traceContext() {
  const traceId = randomBytes(16).toString("hex");
  const parentSpanId = randomBytes(8).toString("hex");
  return {
    traceId,
    parentSpanId,
    traceparent: `00-${traceId}-${parentSpanId}-01`,
  };
}

function traceHeaders(context) {
  return { traceparent: context.traceparent };
}

async function waitForTrace(traceId, predicate) {
  return await poll(`OTLP trace ${traceId}`, () => {
    if (!fs.existsSync(TRACE_FILE)) return [];
    return traceSpans(fs.readFileSync(TRACE_FILE, "utf8"), traceId);
  }, predicate);
}

function hasCompleteLifecycleTrace(spans) {
  const names = new Set(spans.map((span) => span.name));
  const stages = new Set(spans.map((span) => span.attributes["browserpane.runtime.stage"]));
  return REQUIRED_SPANS.every((name) => names.has(name))
    && spans.some((span) => span.serviceName === "bpane-gateway"
      && span.name === "browserpane.http.server"
      && span.attributes["http.route"] === "/api/v1/sessions/{session_id}/automation-access")
    && stages.has("operation")
    && stages.has("create")
    && stages.has("start");
}

function assertTraceContract(spans, callerParentSpanId) {
  const find = (serviceName, name, predicate = () => true) => {
    const match = spans.find((span) => span.serviceName === serviceName
      && span.name === name && predicate(span));
    assert(match, `trace is missing ${serviceName} ${name}`);
    return match;
  };
  const create = find("bpane-runtime-broker", "browserpane.runtime.docker", (span) =>
    span.attributes["browserpane.runtime.stage"] === "create");
  const start = find("bpane-runtime-broker", "browserpane.runtime.docker", (span) =>
    span.attributes["browserpane.runtime.stage"] === "start"
      && span.parentSpanId === create.parentSpanId);
  const execution = find("bpane-runtime-broker", "browserpane.runtime_broker.execute", (span) =>
    span.spanId === create.parentSpanId);
  const policy = find("bpane-runtime-broker", "browserpane.runtime.policy", (span) =>
    span.parentSpanId === execution.spanId
      && span.attributes["browserpane.operation.action"] === "launch");
  const brokerServer = find("bpane-runtime-broker", "browserpane.http.server", (span) =>
    span.spanId === execution.parentSpanId
      && span.attributes["http.route"] === "/v1/operations");
  const authentication = find(
    "bpane-runtime-broker",
    "browserpane.runtime_broker.authenticate",
    (span) => span.parentSpanId === brokerServer.spanId,
  );
  const gatewayClient = find("bpane-gateway", "browserpane.runtime_broker.client", (span) =>
    span.spanId === brokerServer.parentSpanId
      && span.attributes["browserpane.runtime.stage"] === "operation");
  const gatewayServer = find("bpane-gateway", "browserpane.http.server", (span) =>
    span.spanId === gatewayClient.parentSpanId
      && span.attributes["http.route"] === "/api/v1/sessions/{session_id}/automation-access");

  assert.equal(gatewayServer.parentSpanId, callerParentSpanId,
    "gateway did not continue the caller W3C parent");
  assert.equal(gatewayClient.parentSpanId, gatewayServer.spanId,
    "runtime client span is not a child of gateway ingress");
  assert.equal(brokerServer.parentSpanId, gatewayClient.spanId,
    "broker ingress did not continue the runtime client span");
  assert.equal(authentication.parentSpanId, brokerServer.spanId,
    "broker authentication is not a child of broker ingress");
  assert.equal(execution.parentSpanId, brokerServer.spanId,
    "broker execution is not a sibling of broker authentication");
  for (const span of [policy, create, start]) {
    assert.equal(span.parentSpanId, execution.spanId,
      `${span.name} is not a child of broker execution`);
  }
  assert.equal(gatewayServer.attributes["http.response.status_code"], "200");
  assert.equal(brokerServer.attributes["http.response.status_code"], "202");
  for (const span of spans) {
    for (const attribute of Object.keys(span.attributes)) {
      assert(ALLOWED_TRACE_ATTRIBUTES.has(attribute),
        `trace span ${span.name} exported non-allowlisted attribute ${attribute}`);
    }
  }
}

function assertSensitiveValuesAbsent(markers) {
  const exported = fs.readFileSync(TRACE_FILE, "utf8");
  for (const marker of markers.filter(Boolean)) {
    assert(!exported.includes(marker), `sensitive trace marker was exported: ${marker.slice(0, 16)}`);
  }
}

function assertGatewayReady() {
  const result = spawnSync("docker", [
    "exec",
    "bpane-single-node-fixture-gateway-1",
    "curl",
    "-fsS",
    "http://localhost:8932/readyz",
  ], { cwd: ROOT, encoding: "utf8" });
  assert.equal(result.status, 0,
    `gateway readiness failed during collector outage: ${result.stderr.trim()}`);
}

async function waitForCollectorRunning() {
  await poll("collector restart", () => {
    const result = spawnSync("docker", [
      "inspect",
      "--format",
      "{{.State.Running}}",
      "bpane-single-node-fixture-otel-collector-1",
    ], { cwd: ROOT, encoding: "utf8" });
    return result.status === 0 && result.stdout.trim() === "true";
  }, Boolean);
}

async function removeSession(accessToken, sessionId) {
  let response = await fetch(`${API_ORIGIN}/api/v1/sessions/${sessionId}`, {
    method: "DELETE",
    headers: { authorization: `Bearer ${accessToken}` },
  }).catch(() => null);
  if (response?.status === 409) {
    response = await fetch(`${API_ORIGIN}/api/v1/sessions/${sessionId}/kill`, {
      method: "POST",
      headers: { authorization: `Bearer ${accessToken}` },
    }).catch(() => null);
  }
  if (response && !response.ok && response.status !== 404) {
    console.warn(`session cleanup returned HTTP ${response.status}`);
  }
}

async function poll(label, operation, predicate) {
  const started = Date.now();
  let lastError;
  while (Date.now() - started < TIMEOUT_MS) {
    try {
      const value = await operation();
      if (predicate(value)) return value;
    } catch (error) {
      lastError = error;
    }
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  throw new Error(`${label} timed out${lastError ? `: ${lastError.message}` : ""}`);
}

function dockerCompose(args, options = {}) {
  if (options.capture) {
    return execFileSync("docker", [...COMPOSE_ARGS, ...args], {
      cwd: ROOT,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    });
  }
  const result = spawnSync("docker", [...COMPOSE_ARGS, ...args], {
    cwd: ROOT,
    encoding: "utf8",
    stdio: options.allowFailure ? "ignore" : "inherit",
  });
  if (!options.allowFailure) assert.equal(result.status, 0, `docker compose ${args.join(" ")} failed`);
  return result.status;
}

await main();
