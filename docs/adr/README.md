# BrowserPane Architecture Decision Records

Architecture Decision Records preserve decisions that shape several features or
release gates. They do not replace issues or implementation plans.

## Status Values

- Proposed: decision is under review and must not be treated as final.
- Accepted: current implementation and planning should follow the decision.
- Superseded: retained for history and linked to the replacing ADR.
- Rejected: considered and deliberately not selected.

## Records

| ADR | Status | Decision |
| --- | --- | --- |
| `0001-browserpane-product-boundary.md` | Accepted | BrowserPane is a governed browser execution endpoint, not the external BPM system. |
| `0002-delivery-source-of-truth.md` | Accepted | GitHub owns execution state; canonical docs own maturity, gates, risk, and decisions. |
| `0003-remote-protocol-product-contract.md` | Accepted | The custom remote protocol is a product contract requiring specification and conformance. |
| `0004-open-source-license-governance.md` | Proposed | Resolve license and contribution posture explicitly before promotion. |

## Rules

- Number ADRs sequentially.
- Link the canonical issue and affected release gate.
- Record alternatives and consequences, not only the selected result.
- Update or supersede an ADR when implementation evidence changes the decision.
