# ADR 0003: Remote Protocol As A Product Contract

Status: Accepted

Date: 2026-07-31

Related issue: #175

## Context

BrowserPane uses a custom typed protocol over WebTransport for control, tiles,
ROI video, audio, camera, input, files, and shared-session state. Rust and
TypeScript implementations currently interoperate, but compatibility is mostly
implicit in code and the browser client does not enforce the advertised
`SessionReady` version.

## Decision

Treat the BrowserPane remote protocol as a versioned product contract. Publish
its wire behavior, negotiate versions/capabilities, enforce compatibility,
share language-neutral vectors, and test malformed input and parser security.

Describe it as a BrowserPane-specific remote protocol or protocol path. Do not
describe it as an industry standard unless independent standardization and
interoperable implementations exist.

## Consequences

- Gateway/client release compatibility becomes part of #75 release governance.
- Protocol changes require conformance evidence and a compatibility decision.
- Diagnostics can report safe version/capability metadata.
- The implementation remains free to use standard WebTransport and media
  codecs underneath the BrowserPane-specific contract.

## Alternatives Considered

- Treat the protocol as private implementation detail: rejected because it is
  central to embedding, upgrades, and the product differentiation hypothesis.
- Replace it immediately with a third-party remote-desktop protocol: rejected
  without evidence that the same session, policy, media, and automation
  contract can be retained.
