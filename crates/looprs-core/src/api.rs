use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::types::{ModelId, ToolId, ToolName};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    pub role: String,
    pub content: Vec<ContentBlock>,
}

impl Message {
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: vec![ContentBlock::Text { text: text.into() }],
        }
    }

    pub fn assistant(content: Vec<ContentBlock>) -> Self {
        Self {
            role: "assistant".to_string(),
            content,
        }
    }

    pub fn tool_results(results: Vec<ContentBlock>) -> Self {
        Self {
            role: "user".to_string(),
            content: results,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: ToolId,
        name: ToolName,
        input: Value,
    },
    ToolResult {
        tool_use_id: ToolId,
        content: String,
    },
}

#[allow(dead_code)]
#[derive(Serialize)]
pub struct ApiRequest {
    pub model: ModelId,
    pub max_tokens: u32,
    pub system: String,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDefinition>,
}

#[derive(Serialize, Clone, Debug)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn message_user() {
        let msg = Message::user("Hello world");
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content.len(), 1);
        match &msg.content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "Hello world"),
            other => panic!("Expected Text content block, got: {other:?}"),
        }
    }

    #[test]
    fn message_assistant() {
        let content = vec![
            ContentBlock::Text {
                text: "Response".to_string(),
            },
            ContentBlock::ToolUse {
                id: ToolId::new("tool_1"),
                name: ToolName::new("read"),
                input: json!({"path": "/tmp/file"}),
            },
        ];
        let msg = Message::assistant(content);
        assert_eq!(msg.role, "assistant");
        assert_eq!(msg.content.len(), 2);
    }

    #[test]
    fn message_tool_results() {
        let results = vec![ContentBlock::ToolResult {
            tool_use_id: ToolId::new("tool_1"),
            content: "File contents".to_string(),
        }];
        let msg = Message::tool_results(results);
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content.len(), 1);
    }

    #[test]
    fn content_block_serialization_roundtrip() {
        let block = ContentBlock::Text {
            text: "Hello".to_string(),
        };
        let json = serde_json::to_value(&block).expect("serialize");
        assert_eq!(json["type"], "text");
        assert_eq!(json["text"], "Hello");

        let json_str = json!({"type": "text", "text": "Hello"});
        let block: ContentBlock = serde_json::from_value(json_str).expect("deserialize");
        match block {
            ContentBlock::Text { text } => assert_eq!(text, "Hello"),
            other => panic!("Expected Text block, got: {other:?}"),
        }
    }

    #[test]
    fn tool_definition_serialization() {
        let tool = ToolDefinition {
            name: "read".to_string(),
            description: "Read a file".to_string(),
            input_schema: json!({"type": "object", "properties": {"path": {"type": "string"}}}),
        };
        let json = serde_json::to_value(&tool).expect("serialize");
        assert_eq!(json["name"], "read");
    }
}
