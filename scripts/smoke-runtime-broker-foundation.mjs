#!/usr/bin/env node

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const ROOT_DIR = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const COMPOSE = ["compose", "-f", path.join(ROOT_DIR, "deploy", "compose.yml")];
const TOKEN_URL =
  "http://localhost:8091/realms/browserpane-dev/protocol/openid-connect/token";
const MEDIA_TYPE = "application/vnd.browserpane.runtime-broker.v1+json";

async function token(clientId, clientSecret) {
  const body = new URLSearchParams({ grant_type: "client_credentials" });
  const authorization = Buffer.from(`${clientId}:${clientSecret}`).toString("base64");
  const response = await fetch(TOKEN_URL, {
    method: "POST",
    headers: {
      authorization: `Basic ${authorization}`,
      "content-type": "application/x-www-form-urlencoded",
    },
    body,
  });
  assert.equal(response.status, 200, `token request returned ${response.status}`);
  const payload = await response.json();
  assert.equal(typeof payload.access_token, "string");
  return payload.access_token;
}

function brokerRequest({ bearer, body, contentType = MEDIA_TYPE }) {
  const shell = [
    "IFS= read -r token;",
    "curl --silent --show-error --max-time 5",
    "--output /tmp/broker-smoke-body --write-out '%{http_code}'",
    "--request POST",
    `--header 'Content-Type: ${contentType}'`,
    bearer ? "--header \"Authorization: Bearer $token\"" : "",
    "--data-binary @-",
    "http://runtime-broker:8940/v1/operations",
  ]
    .filter(Boolean)
    .join(" ");
  const input = `${bearer ?? "-"}\n${body}`;
  const result = spawnSync(
    "docker",
    [...COMPOSE, "exec", "-T", "gateway", "sh", "-c", shell],
    { cwd: ROOT_DIR, encoding: "utf8", input },
  );
  assert.equal(result.status, 0, result.stderr.trim());
  const response = spawnSync(
    "docker",
    [...COMPOSE, "exec", "-T", "gateway", "cat", "/tmp/broker-smoke-body"],
    { cwd: ROOT_DIR, encoding: "utf8" },
  );
  assert.equal(response.status, 0, response.stderr.trim());
  return { status: Number(result.stdout), body: response.stdout };
}

const request = JSON.stringify({
  api_version: "v1",
  request_id: "019db438-c74a-7ef2-810c-792e298faf00",
  idempotency_key: "browser:launch:foundation-smoke",
  operation: {
    kind: "launch_browser",
    parameters: {
      session_id: "019db438-c74a-7ef2-810c-792e298faf11",
      browser_context_id: null,
    },
  },
});

const validToken = await token(
  "bpane-runtime-broker-gateway",
  "bpane-runtime-broker-gateway-secret",
);
const valid = brokerRequest({ bearer: validToken, body: request });
assert.equal(valid.status, 503);
assert.equal(JSON.parse(valid.body).error.code, "adapter_unavailable");

const unauthenticated = brokerRequest({ body: request });
assert.equal(unauthenticated.status, 401);
assert.equal(JSON.parse(unauthenticated.body).error.code, "authentication_required");

const wrongAudienceToken = await token("bpane-mcp-bridge", "bpane-mcp-bridge-secret");
const wrongAudience = brokerRequest({ bearer: wrongAudienceToken, body: request });
assert.equal(wrongAudience.status, 403);
assert.equal(
  JSON.parse(wrongAudience.body).error.code,
  "authentication_audience_invalid",
);

const wrongMedia = brokerRequest({
  bearer: validToken,
  body: request,
  contentType: "application/json",
});
assert.equal(wrongMedia.status, 415);
assert.equal(JSON.parse(wrongMedia.body).error.code, "unsupported_media_type");

const malformed = brokerRequest({ bearer: validToken, body: "not-json" });
assert.equal(malformed.status, 400);
assert.equal(JSON.parse(malformed.body).error.code, "request_malformed");

assert.ok(!valid.body.includes(validToken));
console.log("Runtime broker foundation authentication and denial smoke passed.");
