#[derive(Clone)]
pub struct MockServer {
    running: bool,
    session_id: String,
    events: Vec<String>,
}

impl Default for MockServer {
    fn default() -> Self {
        Self {
            running: true,
            session_id: "demo-001".to_string(),
            events: vec!["server booted".to_string()],
        }
    }
}

impl MockServer {
    pub fn log(&mut self, event: impl Into<String>) {
        self.events.push(event.into());
    }

    pub fn view(&self) -> String {
        let state = if self.running { "running" } else { "stopped" };
        let mut lines = vec![
            format!("server: {state}"),
            format!("session: {}", self.session_id),
        ];
        lines.extend(
            self.events
                .iter()
                .rev()
                .take(6)
                .rev()
                .map(|e| format!("- {e}")),
        );
        lines.join("\n")
    }
}
