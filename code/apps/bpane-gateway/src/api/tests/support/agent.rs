pub(crate) struct TestAgentServer {
    socket_path: std::path::PathBuf,
    accept_task: tokio::task::JoinHandle<()>,
}

impl TestAgentServer {
    pub(crate) async fn start() -> Self {
        let socket_path = std::path::PathBuf::from(format!(
            "/tmp/bpane-agent-{}.sock",
            uuid::Uuid::now_v7().simple()
        ));
        let _ = std::fs::remove_file(&socket_path);
        let listener = tokio::net::UnixListener::bind(&socket_path).unwrap();
        let accept_task = tokio::spawn(async move {
            let mut connections = Vec::new();
            while let Ok((stream, _)) = listener.accept().await {
                connections.push(stream);
            }
        });

        Self {
            socket_path,
            accept_task,
        }
    }

    pub(crate) fn socket_path(&self) -> String {
        self.socket_path.to_string_lossy().into_owned()
    }
}

impl Drop for TestAgentServer {
    fn drop(&mut self) {
        self.accept_task.abort();
        let _ = std::fs::remove_file(&self.socket_path);
    }
}
