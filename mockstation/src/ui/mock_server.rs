//! Mock WebSocket server for testing.

use anyhow::Result;
use maestro_companion::session::bridge::SessionBridge;
use maestro_test_ws::fixtures;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::net::TcpListener;
use tokio::sync::RwLock;

/// Handle to a running mock server.
#[derive(Debug, Clone)]
pub struct MockServerHandle {
    port: u16,
    bridge: Rc<RwLock<SessionBridge>>,
    browser_connections: Arc<RwLock<HashMap<String, bool>>>,
    cli_connected: Arc<AtomicBool>,
    shutdown_signal: Arc<AtomicBool>,
}

impl MockServerHandle {
    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn bridge(&self) -> Rc<RwLock<SessionBridge>> {
        Rc::clone(&self.bridge)
    }

    pub async fn browser_count(&self) -> usize {
        self.browser_connections.read().await.len()
    }

    pub async fn has_cli_connection(&self) -> bool {
        self.cli_connected.load(Ordering::SeqCst)
    }

    pub async fn register_browser(&self, session_id: String) {
        self.browser_connections
            .write()
            .await
            .insert(session_id, true);
    }

    pub async fn unregister_browser(&self, session_id: &str) {
        self.browser_connections.write().await.remove(session_id);
    }

    pub fn set_cli_connected(&self, connected: bool) {
        self.cli_connected.store(connected, Ordering::SeqCst);
    }

    pub async fn shutdown(&self) -> Result<()> {
        self.shutdown_signal.store(true, Ordering::SeqCst);
        self.browser_connections.write().await.clear();
        self.cli_connected.store(false, Ordering::SeqCst);
        Ok(())
    }
}

/// Mock WebSocket server for integration testing.
#[derive(Debug)]
pub struct MockServer;

impl MockServer {
    pub async fn spawn(buffer_size: usize) -> Result<MockServerHandle> {
        let port = fixtures::PortGenerator::new().next_port();
        let bridge = Rc::new(RwLock::new(SessionBridge::new(buffer_size)));
        let browser_connections = Arc::new(RwLock::new(HashMap::new()));
        let cli_connected = Arc::new(AtomicBool::new(false));
        let shutdown_signal = Arc::new(AtomicBool::new(false));

        let handle = MockServerHandle {
            port,
            bridge: Rc::clone(&bridge),
            browser_connections: Arc::clone(&browser_connections),
            cli_connected: Arc::clone(&cli_connected),
            shutdown_signal: Arc::clone(&shutdown_signal),
        };

        let browser_connections_clone = Arc::clone(&browser_connections);
        let cli_connected_clone = Arc::clone(&cli_connected);
        let shutdown_signal_clone = Arc::clone(&shutdown_signal);

        tokio::spawn(async move {
            let addr = format!("127.0.0.1:{}", port);
            if let Ok(listener) = TcpListener::bind(&addr).await {
                loop {
                    if shutdown_signal_clone.load(Ordering::SeqCst) {
                        break;
                    }
                    match tokio::time::timeout(
                        std::time::Duration::from_millis(100),
                        listener.accept(),
                    )
                    .await
                    {
                        Ok(Ok((stream, _addr))) => {
                            let browser_connections = Arc::clone(&browser_connections_clone);
                            let cli_connected = Arc::clone(&cli_connected_clone);
                            tokio::spawn(async move {
                                let _result = Self::handle_connection_impl(
                                    stream,
                                    browser_connections,
                                    cli_connected,
                                )
                                .await;
                            });
                        }
                        Ok(Err(_)) => break,
                        Err(_) => continue,
                    }
                }
            }
        });

        // Give server task a moment to bind.
        #[allow(clippy::disallowed_methods)]
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        Ok(handle)
    }

    async fn handle_connection_impl(
        stream: tokio::net::TcpStream,
        _browser_connections: Arc<RwLock<HashMap<String, bool>>>,
        _cli_connected: Arc<AtomicBool>,
    ) -> Result<()> {
        let _ws_stream = tokio_tungstenite::accept_async(stream).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "current_thread")]
    async fn mock_server_can_spawn() {
        let handle = MockServer::spawn(100).await.expect("spawn failed");
        assert!(handle.port() > 0);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mock_server_tracks_connections() {
        let handle = MockServer::spawn(512).await.unwrap();
        assert_eq!(handle.browser_count().await, 0);
        assert!(!handle.has_cli_connection().await);
        handle.register_browser("session-1".to_string()).await;
        assert_eq!(handle.browser_count().await, 1);
        handle.set_cli_connected(true);
        assert!(handle.has_cli_connection().await);
        handle.shutdown().await.unwrap();
        assert_eq!(handle.browser_count().await, 0);
        assert!(!handle.has_cli_connection().await);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mock_server_bridge_accessible() {
        let handle = MockServer::spawn(512).await.unwrap();
        let bridge = handle.bridge();
        let _bridge_lock = bridge.read().await;
    }
}
