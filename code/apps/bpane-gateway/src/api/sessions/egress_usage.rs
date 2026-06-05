use super::super::*;

pub(super) async fn report_session_egress_usage(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<ReportSessionEgressUsageRequest>,
) -> Result<Json<SessionEgressUsageResource>, (StatusCode, Json<ErrorResponse>)> {
    authorize_visible_session_request_with_automation_access(&headers, &state, session_id).await?;

    let recorded_at = Utc::now();
    let updated = state
        .session_store
        .record_session_egress_usage(session_id, request.clone())
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("session {session_id} not found"),
                }),
            )
        })?;

    Ok(Json(SessionEgressUsageResource::from_report(
        &updated,
        &request,
        recorded_at,
    )))
}
