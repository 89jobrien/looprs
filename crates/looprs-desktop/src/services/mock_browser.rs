#[derive(Clone, Default)]
pub struct MockBrowser {
    connected: bool,
    inbound: Vec<String>,
    outbound: Vec<String>,
}

impl MockBrowser {
    pub fn connect(&mut self) {
        self.connected = true;
    }

    pub fn disconnect(&mut self) {
        self.connected = false;
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn push_outbound(&mut self, message: impl Into<String>) {
        self.outbound.push(message.into());
    }

    pub fn push_inbound(&mut self, message: impl Into<String>) {
        self.inbound.push(message.into());
    }

    pub fn view(&self) -> String {
        let status = if self.connected {
            "connected"
        } else {
            "disconnected"
        };

        let mut lines = vec![format!("status: {status}")];
        lines.push("outbound:".to_string());
        lines.extend(
            self.outbound
                .iter()
                .rev()
                .take(5)
                .rev()
                .map(|v| format!("- {v}")),
        );
        lines.push("inbound:".to_string());
        lines.extend(
            self.inbound
                .iter()
                .rev()
                .take(5)
                .rev()
                .map(|v| format!("- {v}")),
        );
        lines.join("\n")
    }
}
