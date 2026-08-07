import type {
  WorkflowEventDeliveryAttemptResource,
  WorkflowEventDeliveryListResponse,
  WorkflowEventDeliveryResource,
  WorkflowEventDeliveryState,
  WorkflowEventSubscriptionListResponse,
  WorkflowEventSubscriptionResource,
} from './workflow-event-types';
import { WorkflowEventCatalogError } from './workflow-event-error';

const DELIVERY_STATES = [
  'pending',
  'delivering',
  'delivered',
  'failed',
] satisfies readonly WorkflowEventDeliveryState[];

export class WorkflowEventWireMapper {
  static toSubscriptionList(payload: unknown): WorkflowEventSubscriptionListResponse {
    const object = this.expectRecord(payload, 'workflow event subscription list response');
    return {
      subscriptions: this.expectArray(object.subscriptions, 'workflow event subscriptions').map(
        (value) => this.toSubscription(value),
      ),
    };
  }

  static toSubscription(value: unknown): WorkflowEventSubscriptionResource {
    const object = this.expectRecord(value, 'workflow event subscription');
    return {
      id: this.expectString(object.id, 'workflow event subscription id'),
      name: this.expectString(object.name, 'workflow event subscription name'),
      target_url: this.expectString(object.target_url, 'workflow event subscription target_url'),
      event_types: this.expectArray(
        object.event_types,
        'workflow event subscription event_types',
      ).map((eventType) => this.expectString(eventType, 'workflow event subscription event type')),
      has_signing_secret: this.expectBoolean(
        object.has_signing_secret,
        'workflow event subscription has_signing_secret',
      ),
      deliveries_path: this.expectString(
        object.deliveries_path,
        'workflow event subscription deliveries_path',
      ),
      created_at: this.expectString(object.created_at, 'workflow event subscription created_at'),
      updated_at: this.expectString(object.updated_at, 'workflow event subscription updated_at'),
    };
  }

  static toDeliveryList(payload: unknown): WorkflowEventDeliveryListResponse {
    const object = this.expectRecord(payload, 'workflow event delivery list response');
    return {
      deliveries: this.expectArray(object.deliveries, 'workflow event deliveries').map((value) =>
        this.toDelivery(value),
      ),
    };
  }

  static toDelivery(value: unknown): WorkflowEventDeliveryResource {
    const object = this.expectRecord(value, 'workflow event delivery');
    if (object.payload === undefined || object.payload === null) {
      throw new WorkflowEventCatalogError(
        'workflow event delivery payload must be present.',
        'invalid_payload',
      );
    }
    return {
      id: this.expectString(object.id, 'workflow event delivery id'),
      subscription_id: this.expectString(
        object.subscription_id,
        'workflow event delivery subscription_id',
      ),
      run_id: this.expectString(object.run_id, 'workflow event delivery run_id'),
      event_id: this.expectString(object.event_id, 'workflow event delivery event_id'),
      event_type: this.expectString(object.event_type, 'workflow event delivery event_type'),
      state: this.expectEnum(object.state, DELIVERY_STATES, 'workflow event delivery state'),
      attempt_count: this.expectNumber(
        object.attempt_count,
        'workflow event delivery attempt_count',
      ),
      next_attempt_at:
        this.optionalString(object.next_attempt_at, 'workflow event delivery next_attempt_at') ??
        null,
      last_attempt_at:
        this.optionalString(object.last_attempt_at, 'workflow event delivery last_attempt_at') ??
        null,
      delivered_at:
        this.optionalString(object.delivered_at, 'workflow event delivery delivered_at') ?? null,
      last_response_status:
        this.optionalNumber(
          object.last_response_status,
          'workflow event delivery last_response_status',
        ) ?? null,
      last_error:
        this.optionalString(object.last_error, 'workflow event delivery last_error') ?? null,
      payload: object.payload,
      attempts: this.expectArray(object.attempts, 'workflow event delivery attempts').map((item) =>
        this.toAttempt(item),
      ),
      created_at: this.expectString(object.created_at, 'workflow event delivery created_at'),
      updated_at: this.expectString(object.updated_at, 'workflow event delivery updated_at'),
    };
  }

  private static toAttempt(value: unknown): WorkflowEventDeliveryAttemptResource {
    const object = this.expectRecord(value, 'workflow event delivery attempt');
    return {
      id: this.expectString(object.id, 'workflow event delivery attempt id'),
      delivery_id: this.expectString(
        object.delivery_id,
        'workflow event delivery attempt delivery_id',
      ),
      attempt_number: this.expectNumber(
        object.attempt_number,
        'workflow event delivery attempt_number',
      ),
      response_status:
        this.optionalNumber(
          object.response_status,
          'workflow event delivery attempt response_status',
        ) ?? null,
      error: this.optionalString(object.error, 'workflow event delivery attempt error') ?? null,
      created_at: this.expectString(
        object.created_at,
        'workflow event delivery attempt created_at',
      ),
    };
  }

  private static expectRecord(value: unknown, label: string): Record<string, unknown> {
    if (!value || typeof value !== 'object' || Array.isArray(value)) {
      throw new WorkflowEventCatalogError(`${label} must be an object.`, 'invalid_payload');
    }
    return value as Record<string, unknown>;
  }

  private static expectArray(value: unknown, label: string): readonly unknown[] {
    if (!Array.isArray(value)) {
      throw new WorkflowEventCatalogError(`${label} must be an array.`, 'invalid_payload');
    }
    return value;
  }

  private static expectString(value: unknown, label: string): string {
    if (typeof value !== 'string') {
      throw new WorkflowEventCatalogError(`${label} must be a string.`, 'invalid_payload');
    }
    return value;
  }

  private static optionalString(value: unknown, label: string): string | null | undefined {
    return value === undefined || value === null ? value : this.expectString(value, label);
  }

  private static expectBoolean(value: unknown, label: string): boolean {
    if (typeof value !== 'boolean') {
      throw new WorkflowEventCatalogError(`${label} must be a boolean.`, 'invalid_payload');
    }
    return value;
  }

  private static expectNumber(value: unknown, label: string): number {
    if (typeof value !== 'number' || !Number.isFinite(value)) {
      throw new WorkflowEventCatalogError(`${label} must be a finite number.`, 'invalid_payload');
    }
    return value;
  }

  private static optionalNumber(value: unknown, label: string): number | null | undefined {
    return value === undefined || value === null ? value : this.expectNumber(value, label);
  }

  private static expectEnum<T extends string>(
    value: unknown,
    allowed: readonly T[],
    label: string,
  ): T {
    const candidate = this.expectString(value, label);
    if (!allowed.includes(candidate as T)) {
      throw new WorkflowEventCatalogError(
        `${label} must be one of ${allowed.join(', ')}.`,
        'invalid_payload',
      );
    }
    return candidate as T;
  }
}
