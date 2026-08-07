export type WorkflowEventDeliveryState = 'pending' | 'delivering' | 'delivered' | 'failed';

export type WorkflowEventSubscriptionResource = {
  readonly id: string;
  readonly name: string;
  readonly target_url: string;
  readonly event_types: readonly string[];
  readonly has_signing_secret: boolean;
  readonly deliveries_path: string;
  readonly created_at: string;
  readonly updated_at: string;
};

export type WorkflowEventDeliveryAttemptResource = {
  readonly id: string;
  readonly delivery_id: string;
  readonly attempt_number: number;
  readonly response_status?: number | null;
  readonly error?: string | null;
  readonly created_at: string;
};

export type WorkflowEventDeliveryResource = {
  readonly id: string;
  readonly subscription_id: string;
  readonly run_id: string;
  readonly event_id: string;
  readonly event_type: string;
  readonly state: WorkflowEventDeliveryState;
  readonly attempt_count: number;
  readonly next_attempt_at?: string | null;
  readonly last_attempt_at?: string | null;
  readonly delivered_at?: string | null;
  readonly last_response_status?: number | null;
  readonly last_error?: string | null;
  readonly payload: unknown;
  readonly attempts: readonly WorkflowEventDeliveryAttemptResource[];
  readonly created_at: string;
  readonly updated_at: string;
};

export type WorkflowEventSubscriptionListResponse = {
  readonly subscriptions: readonly WorkflowEventSubscriptionResource[];
};

export type WorkflowEventDeliveryListResponse = {
  readonly deliveries: readonly WorkflowEventDeliveryResource[];
};

export type CreateWorkflowEventSubscriptionRequest = {
  readonly name: string;
  readonly target_url: string;
  readonly event_types: readonly string[];
  readonly signing_secret: string;
};
