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
    fn test_message_user() {
        let msg = Message::user("Hello world");
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content.len(), 1);

        match &msg.content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "Hello world"),
            other => panic!("Expected Text content block, got: {other:?}"),
        }
    }

    #[test]
    fn test_message_user_string() {
        let msg = Message::user(String::from("Test"));
        assert_eq!(msg.role, "user");
        match &msg.content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "Test"),
            other => panic!("Expected Text content block, got: {other:?}"),
        }
    }

    #[test]
    fn test_message_assistant() {
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

        let msg = Message::assistant(content.clone());
        assert_eq!(msg.role, "assistant");
        assert_eq!(msg.content.len(), 2);
    }

    #[test]
    fn test_message_tool_results() {
        let results = vec![ContentBlock::ToolResult {
            tool_use_id: ToolId::new("tool_1"),
            content: "File contents".to_string(),
        }];

        let msg = Message::tool_results(results);
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content.len(), 1);

        match &msg.content[0] {
            ContentBlock::ToolResult {
                tool_use_id,
                content,
            } => {
                assert_eq!(tool_use_id.as_str(), "tool_1");
                assert_eq!(content, "File contents");
            }
            other => panic!("Expected ToolResult content block, got: {other:?}"),
        }
    }

    #[test]
    fn test_content_block_text_serialization() {
        let block = ContentBlock::Text {
            text: "Hello".to_string(),
        };

        let json = serde_json::to_value(&block).expect("serialize ContentBlock");
        assert_eq!(json["type"], "text");
        assert_eq!(json["text"], "Hello");
    }

    #[test]
    fn test_content_block_tool_use_serialization() {
        let block = ContentBlock::ToolUse {
            id: ToolId::new("123"),
            name: ToolName::new("bash"),
            input: json!({"command": "ls"}),
        };

        let json = serde_json::to_value(&block).expect("serialize ContentBlock");
        assert_eq!(json["type"], "tool_use");
        assert_eq!(json["id"], "123");
        assert_eq!(json["name"], "bash");
        assert_eq!(json["input"]["command"], "ls");
    }

    #[test]
    fn test_content_block_tool_result_serialization() {
        let block = ContentBlock::ToolResult {
            tool_use_id: ToolId::new("123"),
            content: "output".to_string(),
        };

        let json = serde_json::to_value(&block).expect("serialize ContentBlock");
        assert_eq!(json["type"], "tool_result");
        assert_eq!(json["tool_use_id"], "123");
        assert_eq!(json["content"], "output");
    }

    #[test]
    fn test_content_block_deserialization() {
        let json = json!({
            "type": "text",
            "text": "Hello"
        });

        let block: ContentBlock = serde_json::from_value(json).expect("deserialize ContentBlock");
        match block {
            ContentBlock::Text { text } => assert_eq!(text, "Hello"),
            other => panic!("Expected Text block, got: {other:?}"),
        }
    }

    #[test]
    fn test_message_serialization_roundtrip() {
        let original = Message::user("Test message");
        let json = serde_json::to_string(&original).expect("serialize Message");
        let deserialized: Message = serde_json::from_str(&json).expect("deserialize Message");

        assert_eq!(deserialized.role, "user");
        assert_eq!(deserialized.content.len(), 1);
    }

    #[test]
    fn test_tool_definition_serialization() {
        let tool = ToolDefinition {
            name: "read".to_string(),
            description: "Read a file".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                }
            }),
        };

        let json = serde_json::to_value(&tool).expect("serialize ToolDefinition");
        assert_eq!(json["name"], "read");
        assert_eq!(json["description"], "Read a file");
        assert_eq!(json["input_schema"]["type"], "object");
    }

    #[test]
    fn test_message_with_multiple_content_blocks() {
        let content = vec![
            ContentBlock::Text {
                text: "I'll read the file".to_string(),
            },
            ContentBlock::ToolUse {
                id: ToolId::new("1"),
                name: ToolName::new("read"),
                input: json!({"path": "test.txt"}),
            },
        ];

        let msg = Message::assistant(content);
        assert_eq!(msg.content.len(), 2);

        match &msg.content[0] {
            ContentBlock::Text { text } => assert!(text.contains("read")),
            other => panic!("Expected Text block first, got: {other:?}"),
        }

        match &msg.content[1] {
            ContentBlock::ToolUse { name, .. } => assert_eq!(name.as_str(), "read"),
            other => panic!("Expected ToolUse block second, got: {other:?}"),
        }
    }

    #[test]
    fn test_api_request_serialization() {
        let req = ApiRequest {
            model: ModelId::new("test-model"),
            max_tokens: 123,
            system: "system".to_string(),
            messages: vec![Message::user("hello")],
            tools: vec![ToolDefinition {
                name: "read".to_string(),
                description: "Read a file".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {"path": {"type": "string"}},
                }),
            }],
        };

        let json = serde_json::to_value(&req).expect("serialize ApiRequest");
        assert_eq!(json["model"], "test-model");
        assert_eq!(json["max_tokens"], 123);
        assert_eq!(json["system"], "system");
        assert!(json["messages"].is_array());
        assert!(json["tools"].is_array());
    }
}
