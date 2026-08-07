import { splitFormEntries } from '$lib/application/admin-form-utils';
import { formatDateTime, type ProjectTone } from '$lib/projects/project-formatters';
import type { CreateWorkflowEventSubscriptionRequest, WorkflowEventDeliveryResource, WorkflowEventDeliveryState, WorkflowEventSubscriptionResource } from './workflow-event-types';

export type WorkflowEventOverviewLoadState =
  | { readonly status: 'loading' }
  | { readonly status: 'error'; readonly message: string }
  | { readonly status: 'ready'; readonly subscriptions: readonly WorkflowEventSubscriptionResource[] };
export type WorkflowEventDetailLoadState =
  | { readonly status: 'idle' }
  | { readonly status: 'loading'; readonly subscriptionId: string }
  | { readonly status: 'error'; readonly subscriptionId: string; readonly message: string }
  | { readonly status: 'ready'; readonly subscription: WorkflowEventSubscriptionResource; readonly deliveries: readonly WorkflowEventDeliveryResource[] };
export type WorkflowEventSubscriptionDraft = { name: string; targetUrl: string; eventTypesText: string; signingSecret: string };
export type WorkflowEventSubscriptionValidation = { readonly valid: boolean; readonly request: CreateWorkflowEventSubscriptionRequest | null; readonly fieldErrors: Readonly<Record<string, readonly string[]>> };

export function buildWorkflowEventOverviewModel(subscriptions: readonly WorkflowEventSubscriptionResource[]) {
  return {
    metrics: [
      metric('total', 'Subscriptions', subscriptions.length),
      metric('signed', 'Signed', subscriptions.filter((subscription) => subscription.has_signing_secret).length),
      metric('wildcard', 'Wildcard filters', subscriptions.filter((subscription) => subscription.event_types.some((type) => type.endsWith('.*'))).length),
      metric('event-types', 'Event filters', subscriptions.reduce((total, subscription) => total + subscription.event_types.length, 0)),
    ],
    rows: subscriptions.map((subscription) => ({
      id: subscription.id,
      name: subscription.name,
      targetUrl: subscription.target_url,
      eventTypes: subscription.event_types.join(', '),
      signing: subscription.has_signing_secret ? 'Signing configured' : 'Signing unavailable',
      signingTone: (subscription.has_signing_secret ? 'success' : 'warning') as ProjectTone,
      updatedAt: formatDateTime(subscription.updated_at),
    })),
  };
}

export function workflowEventSubscriptionMatchesSearch(row: ReturnType<typeof buildWorkflowEventOverviewModel>['rows'][number], query: string): boolean {
  if (!query) return true;
  return [row.id, row.name, row.targetUrl, row.eventTypes, row.signing, row.updatedAt].some((value) => value.toLowerCase().includes(query));
}

export function createWorkflowEventSubscriptionDraft(): WorkflowEventSubscriptionDraft {
  return { name: '', targetUrl: '', eventTypesText: 'workflow_run.created\nworkflow_run.running\nworkflow_run.succeeded\nworkflow_run.failed', signingSecret: '' };
}

export function validateWorkflowEventSubscriptionDraft(draft: WorkflowEventSubscriptionDraft): WorkflowEventSubscriptionValidation {
  const fieldErrors: Record<string, string[]> = {};
  const name = draft.name.trim();
  const targetUrl = draft.targetUrl.trim();
  const eventTypes = splitFormEntries(draft.eventTypesText);
  const signingSecret = draft.signingSecret;
  if (!name) fieldErrors.name = ['Name is required.'];
  if (!isHttpUrl(targetUrl)) fieldErrors.targetUrl = ['Target URL must be an absolute HTTP or HTTPS URL.'];
  if (eventTypes.length === 0) fieldErrors.eventTypes = ['At least one event type is required.'];
  else if (eventTypes.some((eventType) => eventType.includes(' '))) fieldErrors.eventTypes = ['Event types must not contain spaces.'];
  if (!signingSecret.trim()) fieldErrors.signingSecret = ['Signing secret is required.'];
  const valid = Object.keys(fieldErrors).length === 0;
  return { valid, fieldErrors, request: valid ? { name, target_url: targetUrl, event_types: eventTypes, signing_secret: signingSecret } : null };
}

export function buildWorkflowEventDeliverySummary(deliveries: readonly WorkflowEventDeliveryResource[]) {
  return {
    metrics: [
      metric('deliveries', 'Deliveries', deliveries.length),
      metric('delivered', 'Delivered', deliveries.filter((delivery) => delivery.state === 'delivered').length),
      metric('retrying', 'Pending / delivering', deliveries.filter((delivery) => delivery.state === 'pending' || delivery.state === 'delivering').length),
      metric('failed', 'Failed', deliveries.filter((delivery) => delivery.state === 'failed').length),
    ],
    rows: deliveries.map(workflowEventDeliveryRow),
  };
}

export function workflowEventDeliveryRow(delivery: WorkflowEventDeliveryResource) {
  return {
    id: delivery.id,
    eventType: delivery.event_type,
    state: deliveryStateLabel(delivery.state),
    tone: deliveryStateTone(delivery.state),
    attempts: delivery.attempt_count === 1 ? '1 attempt' : `${delivery.attempt_count} attempts`,
    response: delivery.last_response_status ? `HTTP ${delivery.last_response_status}` : 'No response',
    error: delivery.last_error ?? 'No error',
    retry: delivery.next_attempt_at ? formatDateTime(delivery.next_attempt_at) : 'No retry scheduled',
    runId: delivery.run_id,
    eventId: delivery.event_id,
    updatedAt: formatDateTime(delivery.updated_at),
    payloadText: JSON.stringify(delivery.payload, null, 2),
    attemptsDetail: delivery.attempts.map((attempt) => ({
      id: attempt.id,
      label: `Attempt ${attempt.attempt_number}`,
      response: attempt.response_status ? `HTTP ${attempt.response_status}` : 'No response',
      error: attempt.error ?? 'No error',
      createdAt: formatDateTime(attempt.created_at),
    })),
  };
}

function deliveryStateLabel(state: WorkflowEventDeliveryState): string { return state.charAt(0).toUpperCase() + state.slice(1); }
function deliveryStateTone(state: WorkflowEventDeliveryState): ProjectTone { if (state === 'delivered') return 'success'; if (state === 'failed') return 'danger'; if (state === 'pending' || state === 'delivering') return 'warning'; return 'neutral'; }
function metric(key: string, label: string, value: number) { return { label, value: String(value), testId: `workflow-events-metric-${key}` }; }
function isHttpUrl(value: string): boolean { try { const url = new URL(value); return (url.protocol === 'http:' || url.protocol === 'https:') && Boolean(url.host); } catch { return false; } }
