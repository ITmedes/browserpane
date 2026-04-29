use super::super::*;

#[derive(Debug, Clone)]
pub(crate) struct CapturedWebhookRequest {
    pub(crate) headers: HashMap<String, String>,
    pub(crate) body: Value,
}

#[derive(Clone, Default)]
struct TestWebhookReceiverState {
    requests: Arc<Mutex<Vec<CapturedWebhookRequest>>>,
    statuses: Arc<Mutex<Vec<StatusCode>>>,
}

pub(crate) struct TestWebhookReceiver {
    pub(crate) url: String,
    state: TestWebhookReceiverState,
    shutdown: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<()>,
}

impl TestWebhookReceiver {
    pub(crate) async fn start(statuses: Vec<StatusCode>) -> Self {
        let state = TestWebhookReceiverState {
            requests: Arc::new(Mutex::new(Vec::new())),
            statuses: Arc::new(Mutex::new(statuses)),
        };
        let app = axum::Router::new().route(
            "/events",
            axum::routing::post({
                let state = state.clone();
                move |headers: axum::http::HeaderMap, body: axum::body::Bytes| {
                    let state = state.clone();
                    async move {
                        let body = serde_json::from_slice::<Value>(&body).unwrap();
                        let mut captured_headers = HashMap::new();
                        for name in [
                            "x-bpane-event-id",
                            "x-bpane-event-type",
                            "x-bpane-delivery-id",
                            "x-bpane-subscription-id",
                            "x-bpane-signature-timestamp",
                            "x-bpane-signature-v1",
                        ] {
                            if let Some(value) =
                                headers.get(name).and_then(|value| value.to_str().ok())
                            {
                                captured_headers.insert(name.to_string(), value.to_string());
                            }
                        }
                        state.requests.lock().await.push(CapturedWebhookRequest {
                            headers: captured_headers,
                            body,
                        });
                        let status = {
                            let mut statuses = state.statuses.lock().await;
                            if statuses.is_empty() {
                                StatusCode::OK
                            } else {
                                statuses.remove(0)
                            }
                        };
                        (status, "ok")
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test webhook receiver");
        let address = listener.local_addr().expect("receiver addr");
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.await;
                })
                .await
                .expect("run webhook receiver");
        });
        Self {
            url: format!("http://{address}/events"),
            state,
            shutdown: Some(shutdown_tx),
            task,
        }
    }

    pub(crate) async fn requests(&self) -> Vec<CapturedWebhookRequest> {
        self.state.requests.lock().await.clone()
    }
}

impl Drop for TestWebhookReceiver {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.task.abort();
    }
}
