import assert from "node:assert/strict";
import test from "node:test";

import { AdminSecurityHeaderContract } from "./admin-security-header-contract.mjs";

const VALID_HEADERS = `
add_header Content-Security-Policy "base-uri 'self'; object-src 'none'; frame-ancestors 'none'" always;
add_header Permissions-Policy "camera=(self)" always;
add_header Referrer-Policy "no-referrer" always;
add_header X-Content-Type-Options "nosniff" always;
add_header X-Frame-Options "DENY" always;
`;

test("accepts the required admin browser defenses", () => {
  assert.doesNotThrow(() => new AdminSecurityHeaderContract().validate(VALID_HEADERS));
});

test("rejects each missing admin browser defense", () => {
  for (const line of VALID_HEADERS.trim().split("\n")) {
    const weakened = VALID_HEADERS.replace(`${line}\n`, "");
    assert.throws(
      () => new AdminSecurityHeaderContract().validate(weakened),
      /admin security headers must contain/,
    );
  }
});
