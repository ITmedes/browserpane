# Resource Lifecycle Requirements

This document consolidates durable resource lifecycle requirements that were
spread across browser-context, session-template, network/egress, and egress
hardening plans.

## Browser Contexts

Browser contexts turn hidden Chromium profile persistence into explicit
owner-scoped resources.

Required contract:

- create/list/get/delete context resources,
- metadata: id, owner, name, description, labels, state, persistence mode,
  timestamps, and last-used timestamp,
- session create supports fresh/default, ephemeral, and reusable context modes,
- effective context reference is persisted on session resources,
- invalid, cross-owner, deleted, active-writer, or incompatible references are
  rejected,
- reusable contexts mount context-scoped Chromium profile volumes,
- upload/download/session-file data remains session-scoped,
- only one active writer may mutate a reusable context,
- context `last_used_at` updates when sessions start or reconnect with it.

Lifecycle and observability:

- admin/API list and detail show visible session references,
  active-writer count, active-writer session id, storage bytes, retention,
  storage limit, and over-limit state,
- API-backed usage should preserve `profile_storage_limit_exceeded` or the
  equivalent over-limit signal,
- delete is guarded in UI and gateway while sessions or active writers exist,
- docker-backed profile volume data is removed only when delete is safe,
- retention metadata uses `retention_sec` and derived `retention_expires_at`,
- retention cleanup runs on startup and interval, skips active writers, and
  retries later,
- profile storage limit blocks new reusable sessions when inspected usage is
  over limit.

Clone/export/import:

- clone copies metadata and docker-backed profile data into a new reusable
  context,
- clone rejects missing, deleted, ephemeral, cross-owner, or active-writer
  sources,
- export returns an `application/zip` archive with `manifest.json` and optional
  `profile.tar.gz`,
- export rejects missing, deleted, ephemeral, or active-writer sources,
- import creates a new context only; it must not overwrite existing contexts or
  pretend to resume a live browser process,
- import must validate archive size, entry count, manifest, profile archive
  size, and symlink/hardlink safety before extraction.

Admin-new status:

- catalog/create/detail/edit exist,
- clone/import/export UI remains a promotion-parity gap if old admin/API
  parity is required.

Validation examples:

- create owner and project scoped contexts,
- reject malformed labels, blank names, and invalid retention/storage values,
- reject concurrent reusable context writers,
- delete only inactive unused contexts,
- clone inactive context,
- export and import archive,
- verify storage/retention/over-limit evidence,
- run context API, admin, CLI, and smoke checks.

## Session Templates

Session templates are reusable owner-scoped defaults for session creation and
catalog filtering.

Required contract:

- create/list/get/update session-template resources,
- version updates so callers can see template changes over time,
- template defaults cover fields supported by session creation, including
  owner mode, idle timeout, labels, integration context, browser context,
  network identity, egress profile, extensions, and recording policy where the
  API supports them,
- create-session resolves `template_id` and merges caller overrides with
  documented precedence,
- session resources expose template id and effective defaults,
- session catalog filters support template id, state, runtime state, labels,
  and selected integration metadata keys.

Admin-new status:

- templates are selectable in session creation,
- project policy can allowlist templates,
- dedicated template catalog/create/edit route remains missing.

Template smoke:

1. Create template `customer-debug-session` with collaborative owner mode,
   idle timeout, recording mode, and labels.
2. Create a session with that template plus a case-specific label override.
3. Verify inherited and overridden labels/defaults.
4. Query sessions by `template_id` and `label.team`.
5. Query non-matching filters and verify the session is excluded.
6. Confirm admin session list renders sessions created from templates.

## Network Identity And Egress Profiles

Network identity makes session behavior reproducible for region, language,
timezone, browser identity, and approved outbound paths.

Required session/template fields:

- locale,
- language preferences,
- timezone,
- geolocation,
- user agent or approved browser identity override,
- egress profile id.

Required egress profile fields:

- name, description, labels, state,
- proxy metadata,
- bypass rules,
- custom CA reference,
- traffic observation mode,
- sensitive-log sink reference where TLS interception is enabled,
- project id where scoped,
- sanitized effective status and diagnostics.

Runtime requirements:

- explicit session fields override template defaults,
- effective runtime configuration is visible in session/status/detail,
- docker-backed runtimes receive locale/language, timezone, geolocation,
  browser identity, Chromium user-agent, proxy, bypass, and CA metadata,
- TLS interception requires explicit `traffic_observation.mode=tls_intercept`,
  proxy URL, custom CA reference, and approved sensitive-log sink reference,
- custom CA material is materialized only for docker-backed runtimes through
  the runtime boundary,
- disabled or unhealthy profiles must not look like healthy launch choices.

Privacy and secret requirements:

- no raw proxy credentials in session metadata, egress profile payloads,
  diagnostics, labels, UI, CLI output, Docker labels, or normal logs,
- proxy auth uses credential binding references,
- credentials resolve only at runtime launch,
- BrowserPane stores sanitized observer/session/profile/container correlation
  and byte counters only,
- URL/status/timing/header/body/decrypted traffic logs stay with the proxy or
  secure web gateway.

Diagnostics requirements:

- profile reachability probe can run before assigning a profile to a session,
- session diagnostics show config proof, runtime launch metadata, active
  browser probe evidence, public IP/TLS issuer where safe, and last sanitized
  failure reason,
- active session probes run only against already-ready sessions,
- diagnostics must not implicitly start stopped sessions,
- profile and session diagnostics survive refresh/reconnect where persisted.

Local egress presets:

- no egress,
- egress as forward proxy,
- egress as TLS interceptor.

Proxy-auth validation:

- local auth-enforcing proxy fixture validates production-shaped auth flows,
- proxy-auth probes must distinguish missing binding, unavailable provider,
  unresolved/malformed secret, proxy-auth rejection, and success,
- observer logs may show valid/rejected traffic but must not embed secrets,
- API/UI/CLI/Docker labels expose only profile id, session id, observer mode,
  and proxy-auth configured state.

Egress smoke:

1. Start BrowserPane compose and egress observer fixtures.
2. Create metadata-only proxy and TLS-intercept profiles.
3. Clone one profile, edit it, and disable the clone.
4. Run profile reachability diagnostics.
5. Start one session with no egress, one with proxy egress, and one with
   TLS-intercept egress.
6. Confirm live view, session inspector, detail, and CLI show the same effective
   egress state.
7. Run active browser/session egress probes and verify sanitized results.
8. Confirm Squid/mitmproxy logs correlate to the expected session container IP
   through Docker labels.
9. Browse a public HTTPS site through TLS intercept and verify the local test
   CA is visible as intended.
10. Attach a proxy-auth credential binding and confirm no secret leaks.

## Resource Catalog Admin-New Requirements

Every resource catalog should follow the same product pattern:

- backend-backed list,
- search/lenses where useful,
- selected resource metadata,
- create route,
- detail/edit route,
- clone/import/export/probe/disable/delete where supported,
- local field validation,
- inline feedback near the field or action,
- stable disabled reasons,
- API payload or copyable API examples where useful,
- no horizontal overflow on desktop or narrow mobile.
