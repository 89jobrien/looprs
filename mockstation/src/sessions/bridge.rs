//! Session bridge: state machine translating CLI ↔ browser protocol messages.
//!
//! The bridge handles:
//! - Converting CLI protocol messages (assistant, stream_event, control_request) to browser events
//! - Translating browser messages (permission_response, session_subscribe) back to CLI messages
//! - Managing permission request lifecycle and tracking pending permissions
//! - CLI subprocess lifecycle control (start, stop, restart, health check)
//! - Event buffering and replay for browser reconnection scenarios
//!
//! # Architecture
//!
//! The bridge is a single-owner state machine (not `Clone`) to prevent concurrent mutations.
//! It maintains:
//! - `event_buffer`: Circular buffer of recent events (configurable capacity)
//! - `pending_permissions`: Permission requests awaiting browser response
//! - `cli_process`: Subprocess handle for Claude CLI
//!
//! # Message Flow
//!
//! ```text
//! CLI (NDJSON)                        Browser (JSON)
//!     |                                    |
//!     +---- handle_cli_message() -------> |
//!     |     (assistant, stream, result)   |
//!     |                                    |
//!     | <---- handle_browser_message()----+
//!           (permission_response)
//! ```

use crate::protocol::browser::{
    BrowserIncomingBase, BrowserIncomingKnown, BrowserIncomingMessage, BrowserOutgoingKnown,
    BrowserOutgoingMessage,
};
use crate::protocol::cli::{
    CanUseToolRequest, CliMessage, CliMessageKnown, ControlRequest, ControlResponse,
    ControlResponseBody,
};
use crate::session::cli_process::{CliLifecycleEvent, CliProcess};
use crate::session::event_buffer::EventBuffer;
use anyhow::Result;
use serde_json::Value;
use std::collections::BTreeMap;
use std::time::SystemTime;

/// Output messages from bridge operations.
///
/// After processing a CLI or browser message, the bridge emits output messages
/// that should be routed to the appropriate destination(s).
#[derive(Debug, Clone, PartialEq)]
pub struct BridgeOutputs {
    /// Messages to broadcast to all subscribed browsers
    pub to_browsers: Vec<BrowserIncomingMessage>,
    /// Messages to send back to the CLI process
    pub to_cli: Vec<CliMessage>,
}

impl BridgeOutputs {
    pub fn empty() -> Self {
        Self {
            to_browsers: Vec::new(),
            to_cli: Vec::new(),
        }
    }
}

#[derive(Debug)]
struct PendingPermission {
    tool_use_id: String,
    input: Value,
    created_at: SystemTime,
}

impl PendingPermission {
    /// Check if this permission request has expired.
    ///
    /// Default timeout is 5 minutes (300 seconds).
    fn is_expired(&self, timeout_secs: u64) -> bool {
        SystemTime::now()
            .duration_since(self.created_at)
            .map(|elapsed| elapsed.as_secs() > timeout_secs)
            .unwrap_or(true)
    }
}

/// State machine translating CLI ↔ browser protocol messages.
///
/// Single-owner bridge for a single Maestro session. Manages:
/// - Event buffering with configurable circular buffer capacity (default: 512)
/// - Permission request tracking (request_id -> pending input mapping)
/// - CLI subprocess lifecycle (start, stop, restart, health check)
/// - Sequence number assignment for browser message replay
///
/// # Capacity & Replay
///
/// Events are buffered up to `capacity`. When full, oldest events are dropped (FIFO).
/// Browsers can request replay by sending `session_subscribe { last_seq }`, which triggers
/// an `event_replay` response containing all events with `seq > last_seq`.
///
/// # Not Clone
///
/// The bridge is not `Clone` to enforce single-owner semantics and prevent concurrent mutations.
/// Each session has exactly one bridge, owned by the session's `CompanionSession`.
#[derive(Debug)]
pub struct SessionBridge {
    event_buffer: EventBuffer,
    pending_permissions: BTreeMap<String, PendingPermission>,
    last_acked_seq: u64,
    cli_process: Option<CliProcess>,
}

impl SessionBridge {
    pub fn new(event_buffer_capacity: usize) -> Self {
        Self {
            event_buffer: EventBuffer::new(event_buffer_capacity),
            pending_permissions: BTreeMap::new(),
            last_acked_seq: 0,
            cli_process: None,
        }
    }

    /// Last sequence number acknowledged by the browser.
    pub fn last_acked_seq(&self) -> u64 {
        self.last_acked_seq
    }

    /// Remove expired permission requests and emit timeout error responses.
    ///
    /// Default timeout is 5 minutes (300 seconds). Expired permissions are removed
    /// and error control_responses are emitted for the expired request_ids.
    ///
    /// # Returns
    ///
    /// A vector of CLI messages containing error control_responses for expired permissions.
    pub fn cleanup_expired_permissions(&mut self) -> Vec<CliMessage> {
        const DEFAULT_TIMEOUT_SECS: u64 = 300; // 5 minutes

        let mut expired_requests = Vec::new();

        // Collect expired request_ids
        self.pending_permissions.retain(|request_id, perm| {
            if perm.is_expired(DEFAULT_TIMEOUT_SECS) {
                expired_requests.push(request_id.clone());
                false
            } else {
                true
            }
        });

        // Generate error responses for expired permissions
        expired_requests
            .into_iter()
            .map(|request_id| {
                CliMessage::Known(CliMessageKnown::ControlResponse {
                    response: ControlResponse::Error {
                        request_id,
                        error: "permission_request_timed_out".to_string(),
                        pending_permission_requests: None,
                        extra: BTreeMap::new(),
                    },
                    extra: BTreeMap::new(),
                })
            })
            .collect()
    }

    /// Start the CLI process with the given arguments.
    ///
    /// If a process is already running, stops it first (gracefully).
    /// Emits a `cli_started` event with the process PID.
    ///
    /// # Errors
    ///
    /// Returns an error if the process fails to spawn or if stopping an existing
    /// process fails.
    pub async fn start_cli(&mut self, args: Vec<String>) -> Result<BridgeOutputs> {
        let mut process = self.cli_process.take().unwrap_or_default();
        let event = process.start(args).await?;
        self.cli_process = Some(process);

        Ok(self.emit_lifecycle_event(event))
    }

    /// Stop the CLI process.
    ///
    /// If `force` is false, sends SIGTERM and waits up to 5 seconds for graceful shutdown.
    /// If graceful shutdown times out or `force` is true, sends SIGKILL (Unix) or immediate
    /// kill (Windows). Emits a `cli_stopped` or `cli_crashed` event.
    pub async fn stop_cli(&mut self, force: bool) -> Result<BridgeOutputs> {
        let Some(mut process) = self.cli_process.take() else {
            return Ok(BridgeOutputs::empty());
        };

        let event = process.stop(force).await?;
        Ok(self.emit_lifecycle_event(event))
    }

    /// Restart the CLI process with new arguments.
    ///
    /// Stops the current process (if running) then starts a new one.
    /// Emits `cli_stopped` (or `cli_crashed`) followed by `cli_started`.
    pub async fn restart_cli(&mut self, args: Vec<String>) -> Result<BridgeOutputs> {
        let mut process = self.cli_process.take().unwrap_or_default();
        let event = process.restart(args).await?;
        self.cli_process = Some(process);

        Ok(self.emit_lifecycle_event(event))
    }

    /// Check the health of the CLI process.
    pub fn check_cli_health(&self) -> BridgeOutputs {
        let health = self
            .cli_process
            .as_ref()
            .map(|p| p.health())
            .unwrap_or_else(|| crate::session::cli_process::CliHealth {
                alive: false,
                pid: None,
                uptime_secs: None,
            });

        let message = BrowserIncomingBase::CliHealth {
            alive: health.alive,
            pid: health.pid,
            uptime_secs: health.uptime_secs,
            extra: BTreeMap::new(),
        };

        BridgeOutputs {
            to_browsers: vec![BrowserIncomingMessage::Known(BrowserIncomingKnown {
                seq: None,
                message,
            })],
            to_cli: Vec::new(),
        }
    }

    /// Check if the CLI process is running and emit a crash event if it has exited.
    ///
    /// This should be called periodically to detect crashes.
    pub async fn check_cli_status(&mut self) -> Result<Option<BridgeOutputs>> {
        let Some(process) = self.cli_process.as_mut() else {
            return Ok(None);
        };

        if let Some(event) = process.try_wait() {
            self.cli_process = None;
            Ok(Some(self.emit_lifecycle_event(event)))
        } else {
            Ok(None)
        }
    }

    /// Process a message from the CLI and emit corresponding browser events.
    ///
    /// Handles known message types:
    /// - `assistant` → broadcasts `assistant` event with sequence number
    /// - `stream_event` → broadcasts `stream_event` with sequence number
    /// - `result` → broadcasts `result` event
    /// - `control_request` (can_use_tool) → broadcasts `permission_request` to browsers
    ///
    /// Unknown message types are silently ignored (forward compatibility).
    ///
    /// # Returns
    ///
    /// `BridgeOutputs` with `to_browsers` populated (messages to send to subscribed browsers).
    /// `to_cli` is typically empty for CLI-originated messages.
    pub fn handle_cli_message(&mut self, msg: CliMessage) -> Result<BridgeOutputs> {
        let CliMessage::Known(known) = msg else {
            return Ok(BridgeOutputs::empty());
        };

        match known {
            CliMessageKnown::Assistant {
                message,
                parent_tool_use_id,
                ..
            } => {
                let event = BrowserIncomingBase::Assistant {
                    message,
                    parent_tool_use_id: parent_tool_use_id
                        .map(Value::String)
                        .unwrap_or(Value::Null),
                    timestamp: None,
                    extra: BTreeMap::new(),
                };
                Ok(self.emit_browser_event(event))
            }
            CliMessageKnown::StreamEvent {
                event,
                parent_tool_use_id,
                ..
            } => {
                let event = BrowserIncomingBase::StreamEvent {
                    event,
                    parent_tool_use_id: parent_tool_use_id
                        .map(Value::String)
                        .unwrap_or(Value::Null),
                    extra: BTreeMap::new(),
                };
                Ok(self.emit_browser_event(event))
            }
            k @ CliMessageKnown::Result { .. } => {
                let data = serde_json::to_value(&k)?;
                let event = BrowserIncomingBase::Result {
                    data,
                    extra: BTreeMap::new(),
                };
                Ok(self.emit_browser_event(event))
            }
            CliMessageKnown::ControlRequest {
                request_id,
                request:
                    ControlRequest::CanUseTool(CanUseToolRequest {
                        tool_name,
                        input,
                        tool_use_id,
                        ..
                    }),
                ..
            } => {
                // Check for duplicate request_id
                if self.pending_permissions.contains_key(&request_id) {
                    tracing::warn!(
                        request_id = %request_id,
                        "Duplicate permission request_id from CLI, replacing"
                    );
                    if let Some(old) = self.pending_permissions.get(&request_id) {
                        tracing::debug!(
                            request_id = %request_id,
                            old_tool_use_id = %old.tool_use_id,
                            new_tool_use_id = %tool_use_id,
                            "Permission request_id reused"
                        );
                    }
                }

                self.pending_permissions.insert(
                    request_id.clone(),
                    PendingPermission {
                        tool_use_id,
                        input: input.clone(),
                        created_at: SystemTime::now(),
                    },
                );

                let request_value = serde_json::json!({
                    "request_id": request_id,
                    "subtype": "can_use_tool",
                    "tool_name": tool_name,
                    "input": input
                });
                let event = BrowserIncomingBase::PermissionRequest {
                    request: request_value,
                    extra: BTreeMap::new(),
                };
                Ok(self.emit_browser_event(event))
            }
            _ => Ok(BridgeOutputs::empty()),
        }
    }

    /// Process a message from a browser client and emit appropriate outputs.
    ///
    /// Handles known message types:
    /// - `user_message { content }` → forwards to CLI as User message for streaming
    /// - `permission_response` → sends `control_response` back to CLI with behavior and updated input
    /// - `session_subscribe { last_seq }` → returns `event_replay` with buffered events
    /// - `session_ack { last_seq }` → updates internal `last_acked_seq` counter
    /// - `cli_health_check` → returns current CLI health status
    ///
    /// Other message types (interrupt, etc.) are silently ignored
    /// (may be used by other components).
    ///
    /// # Returns
    ///
    /// `BridgeOutputs` with:
    /// - `to_browsers`: Event replay responses (only for `session_subscribe`)
    /// - `to_cli`: User messages, permission responses, or control messages to send back to CLI
    pub fn handle_browser_message(&mut self, msg: BrowserOutgoingMessage) -> Result<BridgeOutputs> {
        let BrowserOutgoingMessage::Known(known) = msg else {
            return Ok(BridgeOutputs::empty());
        };

        match known {
            BrowserOutgoingKnown::UserMessage {
                content,
                session_id,
                images,
                client_msg_id,
                extra,
            } => {
                let mut message = serde_json::Map::new();
                message.insert("role".to_string(), Value::String("user".to_string()));
                message.insert("content".to_string(), Value::String(content));
                if let Some(images) = images {
                    message.insert("images".to_string(), serde_json::to_value(images)?);
                }

                let cli = CliMessage::Known(CliMessageKnown::User {
                    message: Value::Object(message),
                    parent_tool_use_id: None,
                    session_id,
                    uuid: client_msg_id,
                    is_synthetic: None,
                    extra,
                });

                Ok(BridgeOutputs {
                    to_browsers: Vec::new(),
                    to_cli: vec![cli],
                })
            }
            BrowserOutgoingKnown::SessionSubscribe { last_seq, .. } => {
                let events = self.event_buffer.replay_after(last_seq);
                if events.is_empty() {
                let arc_events = self.event_buffer.replay_after(last_seq);
                if arc_events.is_empty() {
                    return Ok(BridgeOutputs::empty());
                }

                // Convert Arc-wrapped events to plain events for serialization
                let events = arc_events
                    .into_iter()
                    .map(|arc_event| (*arc_event).clone())
                    .collect();

                Ok(BridgeOutputs {
                    to_browsers: vec![BrowserIncomingMessage::Known(BrowserIncomingKnown {
                        seq: None,
                        message: BrowserIncomingBase::EventReplay {
                            events,
                            extra: BTreeMap::new(),
                        },
                    })],
                    to_cli: Vec::new(),
                })
            }
            BrowserOutgoingKnown::SessionAck { last_seq, .. } => {
                self.last_acked_seq = last_seq;
                Ok(BridgeOutputs::empty())
            }
            BrowserOutgoingKnown::PermissionResponse {
                request_id,
                behavior,
                updated_input,
                updated_permissions,
                message,
                ..
            } => {
                let pending = self
                    .pending_permissions
                    .remove(&request_id)
                    .ok_or_else(|| anyhow::anyhow!("unknown request_id: {}", request_id))?;

                let mut response = ControlResponseBody {
                    behavior: behavior.clone(),
                    updated_input: None,
                    updated_permissions: None,
                    tool_use_id: Some(pending.tool_use_id),
                    message,
                    interrupt: None,
                    extra: BTreeMap::new(),
                };

                if behavior == "allow" {
                    let updated = if let Some(map) = updated_input {
                        Value::Object(map.into_iter().collect())
                    } else {
                        pending.input
                    };
                    response.updated_input = Some(updated);

                    if let Some(perms) = updated_permissions {
                        response.updated_permissions = Some(Value::Array(perms));
                    }
                }

                if behavior == "deny" {
                    response.interrupt = Some(false);
                }

                let cli = CliMessage::Known(CliMessageKnown::ControlResponse {
                    response: ControlResponse::Success {
                        request_id,
                        response: Some(response),
                        extra: BTreeMap::new(),
                    },
                    extra: BTreeMap::new(),
                });

                Ok(BridgeOutputs {
                    to_browsers: Vec::new(),
                    to_cli: vec![cli],
                })
            }
            BrowserOutgoingKnown::CliHealthCheck { .. } => Ok(self.check_cli_health()),
            _ => Ok(BridgeOutputs::empty()),
        }
    }

    /// Handle async lifecycle commands from the browser.
    ///
    /// These commands require async operations (start/stop/restart), so they
    /// must be handled separately from the main synchronous message handler.
    pub async fn handle_browser_lifecycle_command(
        &mut self,
        msg: BrowserOutgoingMessage,
    ) -> Result<Option<BridgeOutputs>> {
        let BrowserOutgoingMessage::Known(known) = msg else {
            return Ok(None);
        };

        match known {
            BrowserOutgoingKnown::CliStart { args, .. } => {
                let outputs = self.start_cli(args).await?;
                Ok(Some(outputs))
            }
            BrowserOutgoingKnown::CliStop { force, .. } => {
                let outputs = self.stop_cli(force).await?;
                Ok(Some(outputs))
            }
            BrowserOutgoingKnown::CliRestart { args, .. } => {
                let outputs = self.restart_cli(args).await?;
                Ok(Some(outputs))
            }
            _ => Ok(None),
        }
    }

    /// Gracefully shut down the session.
    ///
    /// This will:
    /// 1. Stop the CLI process (graceful or force)
    /// 2. Drain all pending permissions with error responses
    ///
    /// # Arguments
    ///
    /// * `force` - If true, force-kill the CLI process. If false, try graceful SIGTERM.
    ///
    /// # Returns
    ///
    /// A `BridgeOutputs` containing all final messages to be sent to CLI and browsers.
    pub async fn shutdown(&mut self, force: bool) -> Result<BridgeOutputs> {
        let mut outputs = BridgeOutputs::empty();

        // Stop CLI process
        if self.cli_process.is_some() {
            if let Ok(stop_outputs) = self.stop_cli(force).await {
                outputs.to_browsers.extend(stop_outputs.to_browsers);
                outputs.to_cli.extend(stop_outputs.to_cli);
            }
        }

        // Drain pending permissions with error responses
        let drained: Vec<String> = self.pending_permissions.keys().cloned().collect();
        for request_id in drained {
            outputs.to_cli.push(
                CliMessage::Known(CliMessageKnown::ControlResponse {
                    response: ControlResponse::Error {
                        request_id: request_id.clone(),
                        error: "session_closed".to_string(),
                        pending_permission_requests: None,
                        extra: BTreeMap::new(),
                    },
                    extra: BTreeMap::new(),
                })
            );
            self.pending_permissions.remove(&request_id);
        }

        Ok(outputs)
    }

    /// Check if the session is fully shut down.
    ///
    /// Returns true if there's no CLI process running and no pending permissions.
    pub fn is_shutdown(&self) -> bool {
        self.cli_process.is_none() && self.pending_permissions.is_empty()
    }

    fn emit_lifecycle_event(&mut self, event: CliLifecycleEvent) -> BridgeOutputs {
        let message = match event {
            CliLifecycleEvent::Started { pid, timestamp } => BrowserIncomingBase::CliStarted {
                pid,
                timestamp,
                extra: BTreeMap::new(),
            },
            CliLifecycleEvent::Stopped {
                exit_code,
                timestamp,
            } => BrowserIncomingBase::CliStopped {
                exit_code,
                timestamp,
                extra: BTreeMap::new(),
            },
            CliLifecycleEvent::Crashed {
                signal,
                exit_code,
                timestamp,
            } => BrowserIncomingBase::CliCrashed {
                signal,
                exit_code,
                timestamp,
                extra: BTreeMap::new(),
            },
        };

        let buffered = self.event_buffer.push(message);
        BridgeOutputs {
            to_browsers: vec![BrowserIncomingMessage::Known(BrowserIncomingKnown {
                seq: Some(buffered.seq),
                message: buffered.message,
                message: buffered.message.clone(),
            })],
            to_cli: Vec::new(),
        }
    }

    fn emit_browser_event(&mut self, message: BrowserIncomingBase) -> BridgeOutputs {
        let buffered = self.event_buffer.push(message);
        BridgeOutputs {
            to_browsers: vec![BrowserIncomingMessage::Known(BrowserIncomingKnown {
                seq: Some(buffered.seq),
                message: buffered.message,
                message: buffered.message.clone(),
            })],
            to_cli: Vec::new(),
        }
    }
}

impl Drop for SessionBridge {
    fn drop(&mut self) {
        if !self.pending_permissions.is_empty() {
            tracing::warn!(
                count = self.pending_permissions.len(),
                "SessionBridge dropped with {} pending permissions",
                self.pending_permissions.len()
            );
        }

        if let Some(process) = &self.cli_process {
            if process.is_running() {
                if let Some(pid) = process.pid() {
                    tracing::warn!(
                        pid = pid,
                        "SessionBridge dropped with CLI process still running (PID: {})",
                        pid
                    );
                }
            }
        }
    }
}
