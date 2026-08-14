import { RuntimeBrokerBrowserOverlayContract } from "../runtime-broker/runtime-broker-browser-overlay-contract.mjs";
import { AdminSecurityHeaderContract } from "./admin-security-header-contract.mjs";

const THREAT_MODEL_SECTIONS = [
  "## Profiles And Assumptions",
  "## Assets And Data Classes",
  "## Actors And Attacker Model",
  "## Trust Boundaries And Data Flows",
  "## Threat And Control Matrix",
  "## Security Invariants",
  "## Residual Risks",
];

const HARDENING_SECTIONS = [
  "## Responsibility Model",
  "## Required Production Controls",
  "## Local Development Exceptions",
  "## Deployment Gate Checklist",
];

export class ProductionSecurityBaselineContract {
  constructor(options = {}) {
    this.brokerContract =
      options.brokerContract ?? new RuntimeBrokerBrowserOverlayContract();
    this.adminHeaderContract =
      options.adminHeaderContract ?? new AdminSecurityHeaderContract();
  }

  validate(input) {
    this.brokerContract.validate(input.composeConfig);
    this.adminHeaderContract.validate(input.adminHeaderConfig);
    this.expect(
      input.localComposeSource.startsWith("# BrowserPane Local Development Stack"),
      "canonical Compose must remain classified as local development",
    );
    this.requireSections(input.threatModel, THREAT_MODEL_SECTIONS, "threat model");
    this.requireSections(
      input.hardeningBaseline,
      HARDENING_SECTIONS,
      "production security baseline",
    );
    for (const profile of [
      "local development",
      "production-like broker validation",
      "production deployment",
    ]) {
      this.expect(
        input.threatModel.includes(profile),
        `threat model must classify the ${profile} profile`,
      );
    }
  }

  requireSections(document, sections, name) {
    for (const section of sections) {
      this.expect(document.includes(section), `${name} must contain ${section}`);
    }
  }

  expect(condition, message) {
    if (!condition) throw new Error(message);
  }
}
