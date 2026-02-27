use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum CliMessage {
    Known(CliMessageKnown),
    Unknown(Value),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CliMessageKnown {
    ControlRequest {
        request_id: String,
        request: ControlRequest,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    ControlResponse {
        response: ControlResponse,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    ControlCancelRequest {
        request_id: String,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    KeepAlive {},

    User {
        message: Value,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_tool_use_id: Option<String>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        uuid: Option<String>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        is_synthetic: Option<bool>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },

    System {
        subtype: String,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        cwd: Option<String>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        tools: Option<Vec<String>>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        permission_mode: Option<String>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        claude_code_version: Option<String>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },

    Assistant {
        message: Value,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_tool_use_id: Option<String>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        uuid: Option<String>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },

    StreamEvent {
        event: Value,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_tool_use_id: Option<String>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        uuid: Option<String>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },

    Result {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subtype: Option<String>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },

    ToolProgress {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool_use_id: Option<String>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool_name: Option<String>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        elapsed_time_seconds: Option<f64>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        uuid: Option<String>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },

    ToolUseSummary {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        summary: Option<String>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        preceding_tool_use_ids: Option<Vec<String>>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },

    AuthStatus {
        #[serde(
            rename = "isAuthenticating",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        is_authenticating: Option<bool>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        output: Option<Vec<String>>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },

    SystemStatus {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        status: Option<String>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },

    SystemCompactBoundary {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        boundary: Option<String>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },

    SystemTaskNotification {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        task: Option<String>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        status: Option<String>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },

    SystemFilesPersisted {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        paths: Option<Vec<String>>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },

    StreamlinedText {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },

    StreamlinedToolUseSummary {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        summary: Option<String>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool_use_id: Option<String>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },

    UpdateEnvironmentVariables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        variables: Option<BTreeMap<String, Value>>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ControlRequest {
    CanUseTool(CanUseToolRequest),
    Other {
        subtype: String,
        fields: BTreeMap<String, Value>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanUseToolRequest {
    pub tool_name: String,
    pub input: Value,
    pub tool_use_id: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_suggestions: Option<Value>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_path: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_reason: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "subtype", rename_all = "snake_case")]
pub enum ControlResponse {
    Success {
        request_id: String,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        response: Option<ControlResponseBody>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    Error {
        request_id: String,
        error: String,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        pending_permission_requests: Option<Value>,

        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ControlResponseBody {
    pub behavior: String,

    #[serde(
        rename = "updatedInput",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub updated_input: Option<Value>,

    #[serde(
        rename = "updatedPermissions",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub updated_permissions: Option<Value>,

    #[serde(rename = "toolUseID", default, skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interrupt: Option<bool>,

    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl CliMessage {
    pub fn from_json_line(line: &str) -> Result<Self> {
        let value: Value = serde_json::from_str(line)?;
        Ok(Self::from_value(value))
    }

    pub fn from_value(value: Value) -> Self {
        match serde_json::from_value::<CliMessageKnown>(value.clone()) {
            Ok(known) => Self::Known(known),
            Err(_) => Self::Unknown(value),
        }
    }

    pub fn to_value(&self) -> Value {
        match self {
            Self::Known(msg) => {
                serde_json::to_value(msg).unwrap_or_else(|_| Value::Object(serde_json::Map::new()))
            }
            Self::Unknown(value) => value.clone(),
        }
    }

    pub fn unknown_for_test() -> Self {
        Self::Unknown(serde_json::json!({"type": "unknown"}))
    }
}

impl Serialize for CliMessage {
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

impl<'de> Deserialize<'de> for CliMessage {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        Ok(Self::from_value(value))
    }
}

impl Serialize for ControlRequest {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::CanUseTool(req) => {
                let mut map = serde_json::Map::new();
                map.insert(
                    "subtype".to_string(),
                    Value::String("can_use_tool".to_string()),
                );
                let req_value = serde_json::to_value(req).map_err(serde::ser::Error::custom)?;
                if let Value::Object(obj) = req_value {
                    for (k, v) in obj {
                        map.insert(k, v);
                    }
                }
                Value::Object(map).serialize(serializer)
            }
            Self::Other { subtype, fields } => {
                let mut map = serde_json::Map::new();
                map.insert("subtype".to_string(), Value::String(subtype.clone()));
                for (k, v) in fields {
                    map.insert(k.clone(), v.clone());
                }
                Value::Object(map).serialize(serializer)
            }
        }
    }
}

impl<'de> Deserialize<'de> for ControlRequest {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let Value::Object(mut obj) = value else {
            return Err(serde::de::Error::custom(
                "control_request.request must be an object",
            ));
        };

        let subtype = obj
            .remove("subtype")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .ok_or_else(|| {
                serde::de::Error::custom("control_request.request.subtype is required")
            })?;

        if subtype == "can_use_tool" {
            let req_value = Value::Object(obj);
            let req = serde_json::from_value::<CanUseToolRequest>(req_value)
                .map_err(serde::de::Error::custom)?;
            return Ok(Self::CanUseTool(req));
        }

        let mut fields = BTreeMap::new();
        for (k, v) in obj {
            fields.insert(k, v);
        }

        Ok(Self::Other { subtype, fields })
    }
}
