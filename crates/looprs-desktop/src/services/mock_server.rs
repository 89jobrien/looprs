#[derive(Clone)]
pub struct MockServer {
    rest_running: bool,
    session_id: String,
    events: Vec<String>,
}

impl Default for MockServer {
    fn default() -> Self {
        Self {
            rest_running: false,
            session_id: "demo-001".to_string(),
            events: vec!["server booted".to_string()],
        }
    }
}

impl MockServer {
    pub fn start_rest_api(&mut self) {
        self.rest_running = true;
        self.log("rest api started on http://127.0.0.1:7777");
    }

    pub fn stop_rest_api(&mut self) {
        self.rest_running = false;
        self.log("rest api stopped");
    }

    pub fn is_rest_running(&self) -> bool {
        self.rest_running
    }

    pub fn log(&mut self, event: impl Into<String>) {
        self.events.push(event.into());
    }

    pub fn view(&self) -> String {
        let state = if self.rest_running {
            "running"
        } else {
            "stopped"
        };
        let mut lines = vec![
            format!("rest api: {state}"),
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
