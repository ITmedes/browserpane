import assert from "node:assert/strict";
import test from "node:test";

import {
  flattenTraceSpans,
  parseOtlpJsonLines,
  traceSpans,
} from "./otlp-trace-evidence.mjs";

const traceId = "4bf92f3577b34da6a3ce929d0e0e4736";

function batch() {
  return {
    resourceSpans: [{
      resource: { attributes: [{ key: "service.name", value: { stringValue: "bpane-gateway" } }] },
      scopeSpans: [{ spans: [{
        traceId,
        spanId: "00f067aa0ba902b7",
        name: "browserpane.http.server",
        attributes: [
          { key: "http.response.status_code", value: { intValue: "200" } },
          { key: "sampled", value: { boolValue: true } },
        ],
      }] }],
    }],
  };
}

test("parses complete newline-delimited OTLP JSON and ignores a partial tail", () => {
  const content = `${JSON.stringify(batch())}\n{\"resourceSpans\":`;
  assert.deepEqual(parseOtlpJsonLines(content), [batch()]);
});

test("rejects malformed completed OTLP JSON records", () => {
  assert.throws(() => parseOtlpJsonLines("not-json\n"), /completed line 1/u);
});

test("flattens resource and span attributes without string scraping", () => {
  const [span] = flattenTraceSpans([batch()]);
  assert.equal(span.serviceName, "bpane-gateway");
  assert.equal(span.attributes["http.response.status_code"], "200");
  assert.equal(span.attributes.sampled, true);
  assert.deepEqual(traceSpans(`${JSON.stringify(batch())}\n`, traceId), [span]);
});
