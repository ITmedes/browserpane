import { describe, expect, it } from 'vitest';
import {
  browserContextOptionLabel,
  defaultSessionCreateFormState,
  egressProfileKind,
  egressProfileOptionLabel,
  isLocalProxyEgressPreset,
  isLocalTlsInterceptorEgressPreset,
  networkIdentitySummary,
  parseSessionCreateLabels,
  projectOptionLabel,
  projectUsageSummary,
  sessionBrowserContextSummary,
  sessionTemplateDefaultsSummary,
  validateBrowserContextCreateForm,
  validateSessionCreateForm,
} from './session-create-configurator';

const BROWSER_CONTEXT = {
  id: '019df7be-6222-7b00-8c86-9e1f3f8d4a72',
  name: 'Support profile',
  description: null,
  labels: { team: 'support' },
  persistence_mode: 'reusable',
  retention_sec: 172800,
  retention_expires_at: '2026-05-06T18:30:00Z',
  max_profile_storage_bytes: 268435456,
  state: 'ready',
  created_at: '2026-05-04T18:30:00Z',
  updated_at: '2026-05-04T18:30:00Z',
  last_used_at: null,
  deleted_at: null,
} as const;

const EGRESS_PROFILE = {
  id: '019df7be-6222-7b00-8c86-9e1f3f8d4a73',
  name: 'EU support egress',
  description: null,
  labels: { region: 'eu' },
  proxy: { url: 'https://proxy.example:8443' },
  bypass_rules: ['localhost', '*.internal.example'],
  custom_ca: {
    certificate_ref: 'vault://pki/browserpane/eu-support',
    display_name: 'EU support CA',
  },
  traffic_observation: {
    mode: 'tls_intercept',
    sensitive_log_sink_ref: 'siem://browserpane/eu-support',
    sensitive_log_sink_display_name: 'EU support SIEM',
  },
  state: 'ready',
  effective: {
    proxy_configured: true,
    proxy_auth_configured: false,
    bypass_rule_count: 2,
    custom_ca_configured: true,
    observation_mode: 'tls_intercept',
    tls_interception_enabled: true,
    sensitive_log_sink_configured: true,
  },
  diagnostics: {
    profile_id: '019df7be-6222-7b00-8c86-9e1f3f8d4a73',
    profile_name: 'EU support egress',
    profile_state: 'ready',
    health: 'ready',
    observation_mode: 'tls_intercept',
    proof_level: 'configuration',
    runtime_binding: null,
    runtime_assignment: null,
    proxy_configured: true,
    proxy_auth_configured: false,
    bypass_rule_count: 2,
    custom_ca_configured: true,
    tls_interception_enabled: true,
    sensitive_log_sink_configured: true,
    proof: {
      profile_resolved: true,
      profile_ready: true,
      profile_reachability_collected: false,
      profile_reachability_healthy: false,
      profile_reachability_observed_at: null,
      profile_reachability_failure: null,
      proxy_launch_config_expected: true,
      bypass_rules_expected: 2,
      custom_ca_launch_config_expected: true,
      tls_interception_expected: true,
      sensitive_log_sink_declared: true,
      runtime_launch_observed: false,
      active_probe_collected: false,
      observed_public_ip: null,
      observed_tls_issuer: null,
      last_failure_reason: null,
    },
    warnings: [],
    observed_at: '2026-05-04T18:45:00Z',
  },
  created_at: '2026-05-04T18:45:00Z',
  updated_at: '2026-05-04T18:45:00Z',
} as const;

const PROJECT = {
  id: '019df811-91a5-7b00-9fe5-93403ea57f19',
  name: 'Support tenant',
  description: 'Support tenant project',
  labels: { tenant: 'support' },
  quotas: {
    max_active_sessions: 2,
    max_active_workflow_runs: 4,
    max_retained_storage_bytes: 1073741824,
  },
  policy: {
    allowed_session_template_ids: ['template-1'],
    allowed_egress_profile_ids: ['egress-1'],
  },
  state: 'active',
  usage: {
    project_id: '019df811-91a5-7b00-9fe5-93403ea57f19',
    active_sessions: 1,
    max_active_sessions: 2,
    active_workflow_runs: 1,
    max_active_workflow_runs: 4,
    retained_storage_bytes: 268435456,
    max_retained_storage_bytes: 1073741824,
    observed_at: '2026-05-04T18:50:00Z',
  },
  created_at: '2026-05-04T18:50:00Z',
  updated_at: '2026-05-04T18:50:00Z',
} as const;

describe('session create configurator', () => {
  it('builds the backend-default collaborative command', () => {
    const validation = validateSessionCreateForm(defaultSessionCreateFormState());

    expect(validation.errors).toEqual([]);
    expect(validation.command).toEqual({ owner_mode: 'collaborative' });
    expect(validation.preview).toBe(JSON.stringify({ owner_mode: 'collaborative' }, null, 2));
  });

  it('normalizes idle timeout and labels into a create-session payload', () => {
    const validation = validateSessionCreateForm({
      templateId: '',
      ownerMode: 'exclusive_browser_owner',
      idleTimeoutSec: '1800',
      labels: 'case=1234\npurpose=import-repro, suite=admin',
      locale: 'de-DE',
      languages: 'de-DE, en-US',
      timezone: 'Europe/Berlin',
      geolocationLatitude: '52.52',
      geolocationLongitude: '13.405',
      geolocationAccuracyMeters: '100',
      browserIdentity: 'desktop-chromium-stable',
      egressProfileId: EGRESS_PROFILE.id,
      egressProfiles: [EGRESS_PROFILE],
    });

    expect(validation.errors).toEqual([]);
    expect(validation.command).toEqual({
      owner_mode: 'exclusive_browser_owner',
      idle_timeout_sec: 1800,
      network_identity: {
        locale: 'de-DE',
        languages: ['de-DE', 'en-US'],
        timezone: 'Europe/Berlin',
        geolocation: {
          latitude: 52.52,
          longitude: 13.405,
          accuracy_meters: 100,
        },
        browser_identity: 'desktop-chromium-stable',
        egress_profile_id: EGRESS_PROFILE.id,
      },
      labels: {
        case: '1234',
        purpose: 'import-repro',
        suite: 'admin',
      },
    });
  });

  it('validates network identity and egress profile selections', () => {
    const validation = validateSessionCreateForm({
      templateId: '',
      ownerMode: 'collaborative',
      idleTimeoutSec: '',
      labels: '',
      geolocationLatitude: '91',
      geolocationLongitude: '13.405',
      geolocationAccuracyMeters: '0',
      egressProfileId: 'missing-profile',
      egressProfiles: [EGRESS_PROFILE],
    });

    expect(validation.command).toBeNull();
    expect(validation.errors).toEqual([
      'Selected egress profile is not available.',
      'Latitude must be between -90 and 90.',
      'Geolocation accuracy must be greater than zero.',
    ]);
  });

  it('rejects unsupported owner modes and invalid idle timeout values', () => {
    const validation = validateSessionCreateForm({
      templateId: '',
      ownerMode: 'shared',
      idleTimeoutSec: '0',
      labels: '',
    });

    expect(validation.command).toBeNull();
    expect(validation.errors).toEqual([
      'Owner mode "shared" is not supported.',
      'Idle timeout must be a positive whole number of seconds.',
    ]);
    expect(validation.preview).toBe('Fix validation errors to preview the API payload.');
  });

  it('rejects malformed and duplicate labels', () => {
    const errors: string[] = [];

    const labels = parseSessionCreateLabels(
      'case=1234\nmalformed\npurpose=\ncase=5678',
      errors,
    );

    expect(labels).toEqual({ case: '1234' });
    expect(errors).toEqual([
      'Label "malformed" must use key=value.',
      'Label "purpose=" must use non-empty key and value.',
      'Label "case" is duplicated.',
    ]);
  });

  it('includes the selected template id in the create-session payload', () => {
    const validation = validateSessionCreateForm({
      templateId: '019df5c8-3d03-7800-9e5d-79d69d9a21c0',
      ownerMode: '',
      idleTimeoutSec: '',
      labels: '',
    });

    expect(validation.errors).toEqual([]);
    expect(validation.command).toEqual({
      template_id: '019df5c8-3d03-7800-9e5d-79d69d9a21c0',
    });
    expect(validation.preview).toContain('"template_id"');
    expect(validation.preview).not.toContain('"owner_mode"');
  });

  it('adds selected project admission scope to the create-session payload', () => {
    const validation = validateSessionCreateForm({
      projectId: PROJECT.id,
      templateId: '',
      ownerMode: 'collaborative',
      idleTimeoutSec: '',
      labels: '',
      projects: [PROJECT],
    });

    expect(validation.errors).toEqual([]);
    expect(validation.command).toEqual({
      project_id: PROJECT.id,
      owner_mode: 'collaborative',
    });
    expect(validation.preview).toContain('"project_id"');
    expect(projectOptionLabel(PROJECT)).toBe('Support tenant (active, sessions=1/2, workflows=1/4, templates=1, egress=1)');
    expect(projectUsageSummary(PROJECT)).toBe(
      'state=active | sessions=1/2 | workflow_runs=1/4 | storage=268435456/1073741824 | policy=1 templates,1 egress profiles | labels=tenant=support',
    );
  });

  it('rejects unavailable or archived project selections', () => {
    const archivedProject = {
      ...PROJECT,
      state: 'archived',
    } as const;

    const missing = validateSessionCreateForm({
      projectId: '019df811-91a5-7b00-9fe5-93403ea57f20',
      templateId: '',
      ownerMode: 'collaborative',
      idleTimeoutSec: '',
      labels: '',
      projects: [PROJECT],
    });
    const archived = validateSessionCreateForm({
      projectId: archivedProject.id,
      templateId: '',
      ownerMode: 'collaborative',
      idleTimeoutSec: '',
      labels: '',
      projects: [archivedProject],
    });

    expect(missing.command).toBeNull();
    expect(missing.errors).toContain('Selected project is not available.');
    expect(archived.command).toBeNull();
    expect(archived.errors).toContain('Selected project is archived.');
  });

  it('adds reusable browser context bindings to the create-session payload', () => {
    const validation = validateSessionCreateForm({
      templateId: '',
      ownerMode: 'collaborative',
      idleTimeoutSec: '',
      labels: '',
      browserContextMode: 'reusable',
      browserContextId: BROWSER_CONTEXT.id,
      browserContexts: [BROWSER_CONTEXT],
    });

    expect(validation.errors).toEqual([]);
    expect(validation.command).toEqual({
      owner_mode: 'collaborative',
      browser_context: {
        mode: 'reusable',
        context_id: BROWSER_CONTEXT.id,
      },
    });
    expect(validation.preview).toContain('"browser_context"');
  });

  it('adds ephemeral browser context requests without a catalog id', () => {
    const validation = validateSessionCreateForm({
      templateId: '',
      ownerMode: 'collaborative',
      idleTimeoutSec: '',
      labels: '',
      browserContextMode: 'ephemeral',
      browserContextId: '',
    });

    expect(validation.errors).toEqual([]);
    expect(validation.command).toEqual({
      owner_mode: 'collaborative',
      browser_context: { mode: 'ephemeral' },
    });
  });

  it('rejects invalid reusable browser context selections', () => {
    const deletedContext = {
      ...BROWSER_CONTEXT,
      state: 'deleted',
      deleted_at: '2026-05-04T18:40:00Z',
    } as const;

    const missing = validateSessionCreateForm({
      templateId: '',
      ownerMode: 'collaborative',
      idleTimeoutSec: '',
      labels: '',
      browserContextMode: 'reusable',
      browserContextId: '',
      browserContexts: [BROWSER_CONTEXT],
    });
    const deleted = validateSessionCreateForm({
      templateId: '',
      ownerMode: 'collaborative',
      idleTimeoutSec: '',
      labels: '',
      browserContextMode: 'reusable',
      browserContextId: deletedContext.id,
      browserContexts: [deletedContext],
    });
    const invalidMode = validateSessionCreateForm({
      templateId: '',
      ownerMode: 'collaborative',
      idleTimeoutSec: '',
      labels: '',
      browserContextMode: 'fresh',
      browserContextId: BROWSER_CONTEXT.id,
    });

    expect(missing.command).toBeNull();
    expect(missing.errors).toContain('Reusable browser context requires a selected context.');
    expect(deleted.command).toBeNull();
    expect(deleted.errors).toContain('Selected reusable browser context must be ready.');
    expect(invalidMode.command).toBeNull();
    expect(invalidMode.errors).toContain('Browser context id can only be set for reusable mode.');
  });

  it('validates browser context quick-create requests', () => {
    const valid = validateBrowserContextCreateForm({
      name: 'Support profile',
      labels: 'team=support, suite=admin',
      retentionDays: '7',
      maxProfileStorageMb: '256',
    });
    const invalid = validateBrowserContextCreateForm({
      name: '',
      labels: 'bad-label',
      retentionDays: '0',
      maxProfileStorageMb: '0',
    });

    expect(valid.command).toEqual({
      name: 'Support profile',
      labels: { team: 'support', suite: 'admin' },
      persistence_mode: 'reusable',
      retention_sec: 604800,
      max_profile_storage_bytes: 268435456,
    });
    expect(invalid.command).toBeNull();
    expect(invalid.errors).toEqual([
      'Browser context name is required.',
      'Label "bad-label" must use key=value.',
      'Retention days must be a positive whole number.',
      'Max profile storage must be a positive whole number of MB.',
    ]);
  });

  it('summarizes browser context catalog choices for the UI', () => {
    expect(browserContextOptionLabel(BROWSER_CONTEXT)).toBe('Support profile (019df7be...4a72)');
    expect(sessionBrowserContextSummary('reusable', BROWSER_CONTEXT)).toBe(
      'state=ready | persistence=reusable | never used | labels=team=support | retention=2d | storage_limit=256MiB',
    );
    expect(sessionBrowserContextSummary('fresh', null)).toContain('fresh persisted browser profile');
  });

  it('keeps explicit owner-mode overrides when a template is selected', () => {
    const validation = validateSessionCreateForm({
      templateId: '019df5c8-3d03-7800-9e5d-79d69d9a21c0',
      ownerMode: 'collaborative',
      idleTimeoutSec: '',
      labels: '',
    });

    expect(validation.errors).toEqual([]);
    expect(validation.command).toEqual({
      template_id: '019df5c8-3d03-7800-9e5d-79d69d9a21c0',
      owner_mode: 'collaborative',
    });
  });

  it('summarizes selected template defaults for the UI', () => {
    expect(sessionTemplateDefaultsSummary({
      id: '019df5c8-3d03-7800-9e5d-79d69d9a21c0',
      name: 'Support triage',
      description: null,
      labels: {},
      defaults: {
        owner_mode: 'collaborative',
        project_id: PROJECT.id,
        viewport: { width: 1440, height: 900 },
        idle_timeout_sec: 1800,
        labels: { team: 'support' },
        integration_context: { ticket: 'INC-1234' },
        network_identity: {
          locale: 'de-DE',
          languages: ['de-DE'],
          timezone: 'Europe/Berlin',
          egress_profile_id: EGRESS_PROFILE.id,
        },
        recording: { mode: 'manual', format: 'webm' },
      },
      version: 1,
      created_at: '2026-05-04T18:00:00Z',
      updated_at: '2026-05-04T18:00:00Z',
    })).toBe(
      'owner=collaborative | project=019df811...7f19 | idle=1800s | viewport=1440x900 | labels=team=support | integration=ticket | recording=manual | locale=de-DE | languages=de-DE | timezone=Europe/Berlin | egress=019df7be...4a73',
    );
    expect(networkIdentitySummary({
      locale: 'de-DE',
      languages: ['de-DE'],
      timezone: 'Europe/Berlin',
      geolocation: null,
      user_agent: null,
      browser_identity: null,
      egress_profile_id: EGRESS_PROFILE.id,
    }, [EGRESS_PROFILE])).toBe('locale=de-DE | languages=de-DE | timezone=Europe/Berlin | egress=EU support egress');
    expect(egressProfileOptionLabel(EGRESS_PROFILE)).toBe('EU support egress (ready, proxy, TLS inspect, log sink, custom CA, 2 bypass)');
    expect(egressProfileKind(EGRESS_PROFILE)).toBe('tls_interceptor');
    expect(egressProfileKind({
      ...EGRESS_PROFILE,
      id: '019df7be-6222-7b00-8c86-9e1f3f8d4a75',
      name: 'Plain proxy',
      custom_ca: null,
      traffic_observation: { mode: 'metadata_only' },
      effective: {
        proxy_configured: true,
        proxy_auth_configured: false,
        bypass_rule_count: 1,
        custom_ca_configured: false,
        observation_mode: 'metadata_only',
        tls_interception_enabled: false,
        sensitive_log_sink_configured: false,
      },
    })).toBe('proxy');
    expect(isLocalProxyEgressPreset({
      ...EGRESS_PROFILE,
      name: 'Local: Egress as Proxy',
      labels: {},
    })).toBe(true);
    expect(isLocalTlsInterceptorEgressPreset({
      ...EGRESS_PROFILE,
      name: 'Local: Egress as TLS Interceptor',
      labels: {},
    })).toBe(true);
  });
});
