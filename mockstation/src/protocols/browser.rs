use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum BrowserOutgoingMessage {
    Known(BrowserOutgoingKnown),
    Unknown(Value),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BrowserOutgoingKnown {
    UserMessage {
        content: String,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        images: Option<Vec<BrowserOutgoingImage>>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        client_msg_id: Option<String>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    PermissionResponse {
        request_id: String,
        behavior: String,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        updated_input: Option<BTreeMap<String, Value>>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        updated_permissions: Option<Vec<Value>>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        client_msg_id: Option<String>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    SessionSubscribe {
        last_seq: u64,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    SessionAck {
        last_seq: u64,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    Interrupt {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        client_msg_id: Option<String>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    SetModel {
        model: String,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        client_msg_id: Option<String>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    SetPermissionMode {
        mode: String,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        client_msg_id: Option<String>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    McpGetStatus {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        client_msg_id: Option<String>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    McpToggle {
        #[serde(rename = "serverName")]
        server_name: String,
        enabled: bool,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        client_msg_id: Option<String>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    McpReconnect {
        #[serde(rename = "serverName")]
        server_name: String,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        client_msg_id: Option<String>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    McpSetServers {
        servers: BTreeMap<String, Value>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        client_msg_id: Option<String>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    CliStart {
        args: Vec<String>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        client_msg_id: Option<String>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    CliStop {
        #[serde(default)]
        force: bool,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        client_msg_id: Option<String>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    CliRestart {
        args: Vec<String>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        client_msg_id: Option<String>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    CliHealthCheck {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        client_msg_id: Option<String>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrowserOutgoingImage {
    pub media_type: String,
    pub data: String,

    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BrowserIncomingMessage {
    Known(BrowserIncomingKnown),
    Unknown(Value),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrowserIncomingKnown {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seq: Option<u64>,

    #[serde(flatten)]
    pub message: BrowserIncomingBase,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BrowserIncomingBase {
    SessionInit {
        session: Value,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    SessionUpdate {
        session: Value,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    Assistant {
        message: Value,
        parent_tool_use_id: Value,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        timestamp: Option<i64>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    StreamEvent {
        event: Value,
        parent_tool_use_id: Value,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    Result {
        data: Value,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    PermissionRequest {
        request: Value,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    MessageHistory {
        messages: Vec<BrowserIncomingMessage>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    EventReplay {
        events: Vec<BufferedBrowserEvent>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    CliConnected {
        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    CliDisconnected {
        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    Error {
        error: String,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    CliStarted {
        pid: u32,
        timestamp: i64,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    CliStopped {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        exit_code: Option<i32>,
        timestamp: i64,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    CliRestarted {
        pid: u32,
        timestamp: i64,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    CliHealth {
        alive: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        uptime_secs: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pid: Option<u32>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    CliCrashed {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        signal: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        exit_code: Option<i32>,
        timestamp: i64,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BufferedBrowserEvent {
    pub seq: u64,
    pub message: BrowserIncomingBase,

    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl BrowserOutgoingMessage {
    pub fn from_json(text: &str) -> Result<Self> {
        let value: Value = serde_json::from_str(text)?;
        Ok(Self::from_value(value))
    }

    pub fn from_value(value: Value) -> Self {
        match serde_json::from_value::<BrowserOutgoingKnown>(value.clone()) {
            Ok(known) => Self::Known(known),
            Err(_) => Self::Unknown(value),
        }
    }

    pub fn unknown_for_test() -> Self {
        Self::Unknown(serde_json::json!({"type": "unknown"}))
    }
}

impl Serialize for BrowserOutgoingMessage {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Known(msg) => msg.serialize(serializer),
            Self::Unknown(value) => value.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for BrowserOutgoingMessage {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        Ok(Self::from_value(value))
    }
}

impl BrowserIncomingMessage {
    pub fn from_json(text: &str) -> Result<Self> {
        let value: Value = serde_json::from_str(text)?;
        Ok(Self::from_value(value))
    }

    pub fn from_value(value: Value) -> Self {
        match serde_json::from_value::<BrowserIncomingKnown>(value.clone()) {
            Ok(known) => Self::Known(known),
            Err(_) => Self::Unknown(value),
        }
    }

    pub fn unknown_for_test() -> Self {
        Self::Unknown(serde_json::json!({"type": "unknown"}))
    }
}

impl Serialize for BrowserIncomingMessage {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Known(msg) => msg.serialize(serializer),
            Self::Unknown(value) => value.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for BrowserIncomingMessage {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        Ok(Self::from_value(value))
    }
}
