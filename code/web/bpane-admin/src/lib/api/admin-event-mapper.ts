import {
  AdminEventMapper as SharedAdminEventMapper,
  type AdminErrorEvent,
  type AdminEvent as SharedAdminEvent,
  type AdminEventType,
} from '@browserpane/admin-auth';
import { ControlSessionMapper } from './control-session-mapper';
import type { SessionResource } from './control-types';

export type { AdminErrorEvent, AdminEventType };
export type AdminEvent = SharedAdminEvent<SessionResource>;

const mapper = new SharedAdminEventMapper<SessionResource>((payload) =>
  ControlSessionMapper.toSessionResource(payload),
);

export class AdminEventMapper {
  static toEvent(payload: unknown): AdminEvent {
    return mapper.toEvent(payload);
  }
}
