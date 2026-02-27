#[derive(Clone, Default)]
pub struct MockWs {
    terminal_connected: bool,
    browser_connected: bool,
}

impl MockWs {
    pub fn connect_terminal(&mut self) {
        self.terminal_connected = true;
    }

    pub fn disconnect_terminal(&mut self) {
        self.terminal_connected = false;
    }

    pub fn connect_browser(&mut self) {
        self.browser_connected = true;
    }

    pub fn disconnect_browser(&mut self) {
        self.browser_connected = false;
    }

    pub fn route_terminal_to_browser(&self, message: &str) -> Option<String> {
        if self.terminal_connected && self.browser_connected {
            Some(format!("ws frame from terminal: {message}"))
        } else {
            None
        }
    }

    pub fn route_browser_to_terminal(&self, message: &str) -> Option<String> {
        if self.terminal_connected && self.browser_connected {
            Some(format!("ws frame from browser: {message}"))
        } else {
            None
        }
    }

    pub fn status_line(&self) -> String {
        format!(
            "ws links -> terminal: {}, browser: {}",
            if self.terminal_connected {
                "connected"
            } else {
                "disconnected"
            },
            if self.browser_connected {
                "connected"
            } else {
                "disconnected"
            }
        )
    }
}
