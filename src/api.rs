use serde::{Deserialize, Serialize};
use serde_json::Value;

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
        id: String,
        name: String,
        input: Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

#[derive(Serialize)]
pub struct ApiRequest {
    pub model: String,
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
            _ => panic!("Expected Text content block"),
        }
    }

    #[test]
    fn test_message_user_string() {
        let msg = Message::user(String::from("Test"));
        assert_eq!(msg.role, "user");
        match &msg.content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "Test"),
            _ => panic!("Expected Text content block"),
        }
    }

    #[test]
    fn test_message_assistant() {
        let content = vec![
            ContentBlock::Text {
                text: "Response".to_string(),
            },
            ContentBlock::ToolUse {
                id: "tool_1".to_string(),
                name: "read".to_string(),
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
            tool_use_id: "tool_1".to_string(),
            content: "File contents".to_string(),
        }];
        
        let msg = Message::tool_results(results);
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content.len(), 1);
        
        match &msg.content[0] {
            ContentBlock::ToolResult { tool_use_id, content } => {
                assert_eq!(tool_use_id, "tool_1");
                assert_eq!(content, "File contents");
            }
            _ => panic!("Expected ToolResult content block"),
        }
    }

    #[test]
    fn test_content_block_text_serialization() {
        let block = ContentBlock::Text {
            text: "Hello".to_string(),
        };
        
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["type"], "text");
        assert_eq!(json["text"], "Hello");
    }

    #[test]
    fn test_content_block_tool_use_serialization() {
        let block = ContentBlock::ToolUse {
            id: "123".to_string(),
            name: "bash".to_string(),
            input: json!({"command": "ls"}),
        };
        
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["type"], "tool_use");
        assert_eq!(json["id"], "123");
        assert_eq!(json["name"], "bash");
        assert_eq!(json["input"]["command"], "ls");
    }

    #[test]
    fn test_content_block_tool_result_serialization() {
        let block = ContentBlock::ToolResult {
            tool_use_id: "123".to_string(),
            content: "output".to_string(),
        };
        
        let json = serde_json::to_value(&block).unwrap();
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
        
        let block: ContentBlock = serde_json::from_value(json).unwrap();
        match block {
            ContentBlock::Text { text } => assert_eq!(text, "Hello"),
            _ => panic!("Expected Text block"),
        }
    }

    #[test]
    fn test_message_serialization_roundtrip() {
        let original = Message::user("Test message");
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: Message = serde_json::from_str(&json).unwrap();
        
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
        
        let json = serde_json::to_value(&tool).unwrap();
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
                id: "1".to_string(),
                name: "read".to_string(),
                input: json!({"path": "test.txt"}),
            },
        ];
        
        let msg = Message::assistant(content);
        assert_eq!(msg.content.len(), 2);
        
        match &msg.content[0] {
            ContentBlock::Text { text } => assert!(text.contains("read")),
            _ => panic!("Expected Text block first"),
        }
        
        match &msg.content[1] {
            ContentBlock::ToolUse { name, .. } => assert_eq!(name, "read"),
            _ => panic!("Expected ToolUse block second"),
        }
    }
}
