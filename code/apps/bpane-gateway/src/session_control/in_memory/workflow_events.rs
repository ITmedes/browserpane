use super::*;

impl InMemorySessionStore {
    pub(in crate::session_control) async fn queue_workflow_event_deliveries_for_run_event(
        &self,
        run: &StoredWorkflowRun,
        event: &StoredWorkflowRunEvent,
    ) {
        let subscriptions = self.workflow_event_subscriptions.lock().await.clone();
        let planned_deliveries = plan_workflow_event_deliveries(&subscriptions, run, event);
        self.workflow_event_deliveries
            .lock()
            .await
            .extend(planned_deliveries);
    }

    pub(in crate::session_control) async fn create_workflow_event_subscription(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowEventSubscriptionRequest,
    ) -> Result<StoredWorkflowEventSubscription, SessionStoreError> {
        let now = Utc::now();
        let subscription = StoredWorkflowEventSubscription {
            id: Uuid::now_v7(),
            owner_subject: principal.subject.clone(),
            owner_issuer: principal.issuer.clone(),
            name: request.name,
            target_url: request.target_url,
            event_types: request.event_types,
            signing_secret: request.signing_secret,
            created_at: now,
            updated_at: now,
        };
        self.workflow_event_subscriptions
            .lock()
            .await
            .push(subscription.clone());
        Ok(subscription)
    }

    pub(in crate::session_control) async fn list_workflow_event_subscriptions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredWorkflowEventSubscription>, SessionStoreError> {
        let mut subscriptions = self
            .workflow_event_subscriptions
            .lock()
            .await
            .iter()
            .filter(|subscription| {
                subscription.owner_subject == principal.subject
                    && subscription.owner_issuer == principal.issuer
            })
            .cloned()
            .collect::<Vec<_>>();
        subscriptions.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then_with(|| right.id.cmp(&left.id))
        });
        Ok(subscriptions)
    }

    pub(in crate::session_control) async fn get_workflow_event_subscription_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowEventSubscription>, SessionStoreError> {
        Ok(self
            .workflow_event_subscriptions
            .lock()
            .await
            .iter()
            .find(|subscription| {
                subscription.id == id
                    && subscription.owner_subject == principal.subject
                    && subscription.owner_issuer == principal.issuer
            })
            .cloned())
    }

    pub(in crate::session_control) async fn delete_workflow_event_subscription_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowEventSubscription>, SessionStoreError> {
        let mut subscriptions = self.workflow_event_subscriptions.lock().await;
        let Some(index) = subscriptions.iter().position(|subscription| {
            subscription.id == id
                && subscription.owner_subject == principal.subject
                && subscription.owner_issuer == principal.issuer
        }) else {
            return Ok(None);
        };
        let removed = subscriptions.remove(index);
        drop(subscriptions);

        let delivery_ids = {
            let mut deliveries = self.workflow_event_deliveries.lock().await;
            let delivery_ids = deliveries
                .iter()
                .filter(|delivery| delivery.subscription_id == id)
                .map(|delivery| delivery.id)
                .collect::<Vec<_>>();
            deliveries.retain(|delivery| delivery.subscription_id != id);
            delivery_ids
        };
        self.workflow_event_delivery_attempts
            .lock()
            .await
            .retain(|attempt| !delivery_ids.contains(&attempt.delivery_id));
        Ok(Some(removed))
    }

    pub(in crate::session_control) async fn list_workflow_event_deliveries_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        subscription_id: Uuid,
    ) -> Result<Vec<StoredWorkflowEventDelivery>, SessionStoreError> {
        if self
            .get_workflow_event_subscription_for_owner(principal, subscription_id)
            .await?
            .is_none()
        {
            return Ok(Vec::new());
        }
        let mut deliveries = self
            .workflow_event_deliveries
            .lock()
            .await
            .iter()
            .filter(|delivery| delivery.subscription_id == subscription_id)
            .cloned()
            .collect::<Vec<_>>();
        deliveries.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.event_id.cmp(&right.event_id))
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(deliveries)
    }

    pub(in crate::session_control) async fn list_workflow_event_delivery_attempts_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        subscription_id: Uuid,
    ) -> Result<Vec<StoredWorkflowEventDeliveryAttempt>, SessionStoreError> {
        let deliveries = self
            .list_workflow_event_deliveries_for_owner(principal, subscription_id)
            .await?;
        let delivery_ids = deliveries
            .into_iter()
            .map(|delivery| delivery.id)
            .collect::<Vec<_>>();
        let mut attempts = self
            .workflow_event_delivery_attempts
            .lock()
            .await
            .iter()
            .filter(|attempt| delivery_ids.contains(&attempt.delivery_id))
            .cloned()
            .collect::<Vec<_>>();
        attempts.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(attempts)
    }

    pub(in crate::session_control) async fn requeue_inflight_workflow_event_deliveries(
        &self,
    ) -> Result<(), SessionStoreError> {
        let now = Utc::now();
        for delivery in self.workflow_event_deliveries.lock().await.iter_mut() {
            if delivery.state == WorkflowEventDeliveryState::Delivering {
                delivery.state = WorkflowEventDeliveryState::Pending;
                delivery.next_attempt_at = Some(now);
                delivery.updated_at = now;
            }
        }
        Ok(())
    }

    pub(in crate::session_control) async fn claim_due_workflow_event_deliveries(
        &self,
        limit: usize,
        now: DateTime<Utc>,
    ) -> Result<Vec<StoredWorkflowEventDelivery>, SessionStoreError> {
        let mut deliveries = self.workflow_event_deliveries.lock().await;
        let mut due_indexes = deliveries
            .iter()
            .enumerate()
            .filter(|(_, delivery)| {
                delivery.state == WorkflowEventDeliveryState::Pending
                    && delivery
                        .next_attempt_at
                        .map(|value| value <= now)
                        .unwrap_or(true)
            })
            .map(|(index, delivery)| (index, delivery.created_at, delivery.event_id, delivery.id))
            .collect::<Vec<_>>();
        due_indexes.sort_by(|left, right| {
            left.1
                .cmp(&right.1)
                .then_with(|| left.2.cmp(&right.2))
                .then_with(|| left.3.cmp(&right.3))
        });
        let mut due_indexes = due_indexes
            .into_iter()
            .map(|(index, _, _, _)| index)
            .take(limit)
            .collect::<Vec<_>>();
        let mut claimed = Vec::with_capacity(due_indexes.len());
        for index in due_indexes.drain(..) {
            if let Some(delivery) = deliveries.get_mut(index) {
                delivery.state = WorkflowEventDeliveryState::Delivering;
                delivery.updated_at = now;
                claimed.push(delivery.clone());
            }
        }
        Ok(claimed)
    }

    pub(in crate::session_control) async fn record_workflow_event_delivery_attempt(
        &self,
        delivery_id: Uuid,
        request: RecordWorkflowEventDeliveryAttemptRequest,
    ) -> Result<Option<StoredWorkflowEventDelivery>, SessionStoreError> {
        let now = request.attempted_at;
        let mut deliveries = self.workflow_event_deliveries.lock().await;
        let Some(delivery) = deliveries
            .iter_mut()
            .find(|delivery| delivery.id == delivery_id)
        else {
            return Ok(None);
        };
        delivery.state = request.state;
        delivery.attempt_count = request.attempt_number;
        delivery.next_attempt_at = request.next_attempt_at;
        delivery.last_attempt_at = Some(now);
        delivery.delivered_at = request.delivered_at;
        delivery.last_response_status = request.response_status;
        delivery.last_error = request.error.clone();
        delivery.updated_at = now;
        let updated = delivery.clone();
        drop(deliveries);

        self.workflow_event_delivery_attempts.lock().await.push(
            StoredWorkflowEventDeliveryAttempt {
                id: Uuid::now_v7(),
                delivery_id,
                attempt_number: request.attempt_number,
                response_status: request.response_status,
                error: request.error,
                created_at: now,
            },
        );
        Ok(Some(updated))
    }
}
