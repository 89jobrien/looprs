use crate::services::mock_browser::MockBrowser;
use crate::services::mock_server::MockServer;
use crate::services::mock_terminal::MockTerminal;
use crate::services::mock_ws::MockWs;

#[derive(Clone, Default)]
pub struct MockstationSnapshot {
    pub terminal_view: String,
    pub browser_view: String,
    pub transport_log: String,
}

pub struct MockstationRuntime {
    seq: u64,
    terminal: MockTerminal,
    browser: MockBrowser,
    ws: MockWs,
    server: MockServer,
    transport_events: Vec<String>,
}

impl MockstationRuntime {
    pub fn new() -> Self {
        let mut server = MockServer::default();
        server.log("ready for connect/disconnect actions");

        Self {
            seq: 0,
            terminal: MockTerminal::default(),
            browser: MockBrowser::default(),
            ws: MockWs::default(),
            server,
            transport_events: vec!["#000 mockstation initialized".to_string()],
        }
    }

    pub fn connect_terminal(&mut self) {
        self.terminal.connect();
        self.ws.connect_terminal();
        self.log("terminal connected");
        self.server.log("terminal attached to session");
    }

    pub fn disconnect_terminal(&mut self) {
        self.terminal.disconnect();
        self.ws.disconnect_terminal();
        self.log("terminal disconnected");
        self.server.log("terminal detached from session");
    }

    pub fn connect_browser(&mut self) {
        self.browser.connect();
        self.ws.connect_browser();
        self.log("browser connected");
        self.server.log("browser attached to session");
    }

    pub fn disconnect_browser(&mut self) {
        self.browser.disconnect();
        self.ws.disconnect_browser();
        self.log("browser disconnected");
        self.server.log("browser detached from session");
    }

    pub fn send_from_terminal(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.terminal.push_outbound(message.clone());

        if let Some(frame) = self.ws.route_terminal_to_browser(&message) {
            self.browser.push_inbound(frame.clone());
            self.log(format!("terminal -> browser routed: {frame}"));
        } else {
            self.log(format!("terminal message dropped (link down): {message}"));
        }
    }

    pub fn send_from_browser(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.browser.push_outbound(message.clone());

        if let Some(frame) = self.ws.route_browser_to_terminal(&message) {
            self.terminal.push_inbound(frame.clone());
            self.log(format!("browser -> terminal routed: {frame}"));
        } else {
            self.log(format!("browser message dropped (link down): {message}"));
        }
    }

    pub fn snapshot(&self) -> MockstationSnapshot {
        MockstationSnapshot {
            terminal_view: self.terminal.view(),
            browser_view: self.browser.view(),
            transport_log: {
                let mut lines = vec![self.ws.status_line(), self.server.view()];
                lines.extend(self.transport_events.iter().rev().take(12).rev().cloned());
                lines.join("\n")
            },
        }
    }

    fn log(&mut self, message: impl Into<String>) {
        self.seq += 1;
        self.transport_events
            .push(format!("#{:03} {}", self.seq, message.into()));
    }
}

pub fn build_mockstation_runtime() -> MockstationRuntime {
    MockstationRuntime::new()
}

pub fn build_mockstation_snapshot() -> MockstationSnapshot {
    MockstationRuntime::new().snapshot()
}
