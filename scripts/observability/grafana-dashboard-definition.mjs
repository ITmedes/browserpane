export const GRAFANA_IMAGE = 'grafana/grafana-oss@sha256:'
  + '5dad0df181cb644a14e13617b913b261a54f7d4fd4510721dba420929f35bea2';
export const DASHBOARD_UID = 'browserpane-operations';
export const DATASOURCE_UID = 'browserpane-prometheus';

export const EXPECTED_PANELS = [
  { id: 1, title: 'Scope and interpretation', type: 'text' },
  { id: 2, title: 'Gateway scrape health', type: 'stat', unit: 'short', expr: 'max(up{job="browserpane-gateway"})' },
  { id: 3, title: 'Gateway request rate', type: 'timeseries', unit: 'reqps', expr: 'browserpane:gateway_http_requests:rate5m' },
  { id: 4, title: 'Gateway server-error ratio', type: 'timeseries', unit: 'percentunit', expr: 'browserpane:gateway_http_5xx:ratio_rate5m' },
  { id: 5, title: 'Gateway p95 response duration', type: 'timeseries', unit: 's', expr: 'browserpane:gateway_http_request_duration_seconds:p95_rate5m' },
  { id: 6, title: 'Runtime assignment utilization', type: 'gauge', unit: 'percentunit', expr: 'browserpane:runtime_capacity_utilization:ratio' },
  { id: 7, title: 'Active runtime assignments', type: 'stat', unit: 'short', expr: 'browserpane_gateway_runtime_active_assignments' },
  { id: 8, title: 'Starting runtime assignments', type: 'stat', unit: 'short', expr: 'browserpane_gateway_runtime_starting_assignments' },
  { id: 9, title: 'Runtime assignment limit', type: 'stat', unit: 'short', expr: 'browserpane_gateway_runtime_assignment_limit' },
  { id: 10, title: 'Produced-file upload failure ratio', type: 'stat', unit: 'percentunit', expr: 'browserpane:workflow_produced_file_upload_failure:ratio_increase15m' },
  { id: 11, title: 'Produced-file upload failures', type: 'stat', unit: 'short', expr: 'browserpane:workflow_produced_file_upload_failures:increase15m' },
  { id: 12, title: 'Event-delivery success ratio', type: 'stat', unit: 'percentunit', expr: 'browserpane:workflow_event_delivery_success:ratio_increase15m' },
  { id: 13, title: 'Event-delivery retries', type: 'stat', unit: 'short', expr: 'browserpane:workflow_event_delivery_retries:increase15m' },
  { id: 14, title: 'Event-delivery terminal failures', type: 'stat', unit: 'short', expr: 'browserpane:workflow_event_delivery_failures:increase15m' },
  { id: 15, title: 'Recording finalization success ratio', type: 'stat', unit: 'percentunit', expr: 'browserpane:recording_artifact_finalize_success:ratio_increase15m' },
  { id: 16, title: 'Recording finalization failures', type: 'stat', unit: 'short', expr: 'browserpane:recording_artifact_finalize_failures:increase15m' },
  { id: 17, title: 'Recording worker failures', type: 'stat', unit: 'short', expr: 'browserpane:recording_failures:increase15m' },
  { id: 18, title: 'Playback export failures', type: 'stat', unit: 'short', expr: 'browserpane:recording_playback_export_failures:increase15m' },
  { id: 19, title: 'Workflow retention failures', type: 'stat', unit: 'short', expr: 'browserpane:workflow_retention_failures:increase1h' },
  { id: 20, title: 'Recording retention failures', type: 'stat', unit: 'short', expr: 'browserpane:recording_retention_failures:increase1h' },
];

export const FORBIDDEN_QUERY_FRAGMENTS = [
  'owner_id', 'project_id', 'session_id', 'workflow_id', 'recording_id',
  'target_url', 'authorization', 'bearer', 'credential', 'payload',
  'artifact_ref', 'raw_error', 'browser_content', '{{', '$',
];
