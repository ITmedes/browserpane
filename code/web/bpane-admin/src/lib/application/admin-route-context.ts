import type { AdminEventClient } from '../api/admin-event-client';
import type { ControlClient } from '../api/control-client';
import type { WorkflowClient } from '../api/workflow-client';
import type { AuthConfig } from '../auth/auth-config';
import type { AuthSnapshot } from '../auth/oidc-types';

export type AdminRouteContext = {
  readonly auth: AuthSnapshot;
  readonly authConfig: AuthConfig | null;
  readonly controlClient: ControlClient;
  readonly adminEventClient: AdminEventClient;
  readonly workflowClient: WorkflowClient;
  readonly adminOpen: boolean;
  readonly setAdminOpen: (open: boolean) => void;
};
