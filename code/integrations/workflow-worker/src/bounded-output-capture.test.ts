import assert from "node:assert/strict";
import test from "node:test";

import { BoundedOutputCapture } from "./bounded-output-capture.js";

test("preserves output at the exact byte limit", () => {
  const capture = new BoundedOutputCapture(5);

  capture.append(Buffer.from("hello"));

  assert.deepEqual(capture.result(), {
    text: "hello",
    capturedBytes: 5,
    omittedBytes: 0,
    truncated: false,
  });
});

test("retains only the newest output and reports omitted bytes", () => {
  const capture = new BoundedOutputCapture(5);

  capture.append(Buffer.from("abc"));
  capture.append(Buffer.from("defgh"));

  assert.deepEqual(capture.result(), {
    text: "[BrowserPane: omitted 3 earlier output bytes]\ndefgh",
    capturedBytes: 5,
    omittedBytes: 3,
    truncated: true,
  });
});

test("drops a partial leading UTF-8 sequence after ring truncation", () => {
  const capture = new BoundedOutputCapture(4);

  capture.append(Buffer.from("x€yz", "utf8"));

  const result = capture.result();
  assert.equal(result.text, "[BrowserPane: omitted 2 earlier output bytes]\nyz");
  assert.doesNotMatch(result.text, /�/u);
});

test("rejects an invalid output limit", () => {
  assert.throws(() => new BoundedOutputCapture(0), /positive integer/u);
});
