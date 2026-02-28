//! Browser WebSocket client that mirrors the web UI behavior.

use anyhow::{Context, Result};
use futures::{SinkExt, StreamExt};
use maestro_companion::protocol::browser::{
    BrowserIncomingBase, BrowserIncomingMessage, BrowserOutgoingKnown, BrowserOutgoingMessage,
};
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{Mutex, RwLock, mpsc};
use tokio_tungstenite::tungstenite::Message;

/// WebSocket browser client for integration testing.
#[derive(Debug)]
pub struct BrowserWsClient {
    url: String,
    ws: Arc<Mutex<Option<maestro_test_ws::WsStream>>>,
    event_buffer: Arc<RwLock<Vec<BrowserIncomingMessage>>>,
    last_seq: Arc<AtomicU64>,
    tx: mpsc::Sender<BrowserOutgoingMessage>,
}

impl BrowserWsClient {
    pub async fn connect(url: &str) -> Result<Self> {
        let ws = maestro_test_ws::connect_companion_browser_ws(url, "").await?;
        let ws = Arc::new(Mutex::new(Some(ws)));
        let (tx, rx) = mpsc::channel::<BrowserOutgoingMessage>(64);
        let event_buffer = Arc::new(RwLock::new(Vec::new()));
        let last_seq = Arc::new(AtomicU64::new(0));

        let reader_ws = Arc::clone(&ws);
        let reader_buffer = Arc::clone(&event_buffer);
        let reader_last_seq = Arc::clone(&last_seq);
        tokio::spawn(async move {
            Self::reader_loop(reader_ws, reader_buffer, reader_last_seq).await;
        });

        let writer_ws = Arc::clone(&ws);
        tokio::spawn(async move {
            Self::writer_loop(writer_ws, rx).await;
        });

        Ok(Self {
            url: url.to_string(),
            ws,
            event_buffer,
            last_seq,
            tx,
        })
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn last_seq(&self) -> u64 {
        self.last_seq.load(Ordering::SeqCst)
    }

    pub async fn event_count(&self) -> usize {
        self.event_buffer.read().await.len()
    }

    pub async fn events(&self) -> Vec<BrowserIncomingMessage> {
        self.event_buffer.read().await.clone()
    }

    pub async fn send_user_message(&self, content: String) -> Result<()> {
        let msg = BrowserOutgoingMessage::Known(BrowserOutgoingKnown::UserMessage {
            content,
            session_id: None,
            images: None,
            client_msg_id: None,
            extra: Default::default(),
        });
        self.tx.send(msg).await.context("send user message")?;
        Ok(())
    }

    pub async fn send_permission_response(
        &self,
        request_id: String,
        behavior: String,
        updated_input: Option<Value>,
    ) -> Result<()> {
        let updated_input = updated_input.map(|v| match v {
            Value::Object(map) => map.into_iter().collect(),
            other => {
                let mut map = serde_json::Map::new();
                map.insert("value".to_string(), other);
                map.into_iter().collect()
            }
        });
        let msg = BrowserOutgoingMessage::Known(BrowserOutgoingKnown::PermissionResponse {
            request_id,
            behavior,
            updated_input,
            updated_permissions: None,
            message: None,
            client_msg_id: None,
            extra: Default::default(),
        });
        self.tx
            .send(msg)
            .await
            .context("send permission response")?;
        Ok(())
    }

    pub async fn send_session_subscribe(&self) -> Result<()> {
        let msg = BrowserOutgoingMessage::Known(BrowserOutgoingKnown::SessionSubscribe {
            last_seq: self.last_seq(),
            extra: Default::default(),
        });
        self.tx.send(msg).await.context("send session subscribe")?;
        Ok(())
    }

    async fn reader_loop(
        ws: Arc<Mutex<Option<maestro_test_ws::WsStream>>>,
        event_buffer: Arc<RwLock<Vec<BrowserIncomingMessage>>>,
        last_seq: Arc<AtomicU64>,
    ) {
        loop {
            let msg = {
                let mut guard = ws.lock().await;
                let Some(sock) = guard.as_mut() else {
                    return;
                };
                sock.next().await
            };

            let Some(Ok(Message::Text(text))) = msg else {
                continue;
            };

            let parsed: Result<BrowserIncomingMessage> = BrowserIncomingMessage::from_json(&text);
            let Ok(parsed) = parsed else {
                continue;
            };

            if let BrowserIncomingMessage::Known(known) = &parsed {
                if let Some(seq) = known.seq {
                    let current = last_seq.load(Ordering::SeqCst);
                    if seq > current {
                        last_seq.store(seq, Ordering::SeqCst);
                    }
                }
            }

            event_buffer.write().await.push(parsed);
        }
    }

    async fn writer_loop(
        ws: Arc<Mutex<Option<maestro_test_ws::WsStream>>>,
        mut rx: mpsc::Receiver<BrowserOutgoingMessage>,
    ) {
        while let Some(msg) = rx.recv().await {
            let text = match serde_json::to_string(&msg) {
                Ok(text) => text,
                Err(_) => continue,
            };
            let mut guard = ws.lock().await;
            let Some(sock) = guard.as_mut() else {
                return;
            };
            let _ = sock.send(Message::Text(text)).await;
        }
    }

    pub async fn assert_received_permission_request(&self) -> bool {
        self.event_buffer.read().await.iter().any(|e| match e {
            BrowserIncomingMessage::Known(msg) => {
                matches!(msg.message, BrowserIncomingBase::PermissionRequest { .. })
            }
            _ => false,
        })
    }
}
