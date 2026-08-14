import assert from "node:assert/strict";
import test from "node:test";

import { ProductionSecurityBaselineContract } from "./production-security-baseline-contract.mjs";

class RecordingContract {
  calls = [];

  validate(value) {
    this.calls.push(value);
  }
}

function validInput() {
  return {
    composeConfig: { services: {} },
    localComposeSource: "# BrowserPane Local Development Stack\nservices: {}\n",
    adminHeaderConfig: "admin headers",
    threatModel: [
      "## Profiles And Assumptions",
      "local development",
      "production-like broker validation",
      "production deployment",
      "## Assets And Data Classes",
      "## Actors And Attacker Model",
      "## Trust Boundaries And Data Flows",
      "## Threat And Control Matrix",
      "## Security Invariants",
      "## Residual Risks",
    ].join("\n"),
    hardeningBaseline: [
      "## Responsibility Model",
      "## Required Production Controls",
      "## Local Development Exceptions",
      "## Deployment Gate Checklist",
    ].join("\n"),
  };
}

function contract() {
  const brokerContract = new RecordingContract();
  const adminHeaderContract = new RecordingContract();
  return {
    baseline: new ProductionSecurityBaselineContract({
      brokerContract,
      adminHeaderContract,
    }),
    brokerContract,
    adminHeaderContract,
  };
}

test("composes broker, admin, profile, and document evidence", () => {
  const { baseline, brokerContract, adminHeaderContract } = contract();
  const input = validInput();

  baseline.validate(input);

  assert.deepEqual(brokerContract.calls, [input.composeConfig]);
  assert.deepEqual(adminHeaderContract.calls, [input.adminHeaderConfig]);
});

test("rejects missing profile classifications and required sections", () => {
  for (const missing of [
    "# BrowserPane Local Development Stack",
    "production-like broker validation",
    "## Threat And Control Matrix",
    "## Deployment Gate Checklist",
  ]) {
    const { baseline } = contract();
    const input = validInput();
    for (const key of ["localComposeSource", "threatModel", "hardeningBaseline"]) {
      input[key] = input[key].replace(missing, "removed");
    }
    assert.throws(() => baseline.validate(input));
  }
});
