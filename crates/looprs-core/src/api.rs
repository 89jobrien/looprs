use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::types::{ModelId, ToolId, ToolName};

/// One turn in a conversation: a role plus its ordered content blocks.
///
/// A single message may mix prose and tool activity, so [`Message::content`]
/// is a list rather than a string.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    /// Wire role, one of `"user"` or `"assistant"`.
    ///
    /// Stored as a `String` rather than an enum to stay forward-compatible
    /// with provider-specific roles.
    pub role: String,
    /// Ordered content blocks; order is significant and is preserved when
    /// sent to the provider.
    pub content: Vec<ContentBlock>,
}

impl Message {
    /// Build a `user` message containing a single [`ContentBlock::Text`].
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: vec![ContentBlock::Text { text: text.into() }],
        }
    }

    /// Build an `assistant` message from pre-built content blocks.
    ///
    /// Used to replay a model response, which may interleave
    /// [`ContentBlock::Text`] and [`ContentBlock::ToolUse`] blocks.
    pub fn assistant(content: Vec<ContentBlock>) -> Self {
        Self {
            role: "assistant".to_string(),
            content,
        }
    }

    /// Build the message that returns tool output to the model.
    ///
    /// Note the role is `"user"`, not a dedicated tool role: tool results are
    /// modelled as user-turn content (the Anthropic convention). Callers
    /// targeting OpenAI-style APIs must translate this into separate `tool`
    /// role messages. `results` is expected to hold
    /// [`ContentBlock::ToolResult`] blocks, one per outstanding tool call,
    /// but this is not enforced.
    pub fn tool_results(results: Vec<ContentBlock>) -> Self {
        Self {
            role: "user".to_string(),
            content: results,
        }
    }
}

/// A single unit of message content.
///
/// Serializes with an external `type` tag in snake_case (`text`, `tool_use`,
/// `tool_result`), matching the Anthropic wire format.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Plain prose from either participant.
    Text {
        /// The text body.
        text: String,
    },
    /// A model request to invoke a tool.
    ///
    /// Pair the [`ToolId`] with the [`ContentBlock::ToolResult`] that answers
    /// it; providers reject a response whose IDs do not match.
    ToolUse {
        /// Correlation ID echoed back in the matching `ToolResult`.
        id: ToolId,
        /// Name of the tool to invoke.
        name: ToolName,
        /// Tool arguments, shaped by that tool's input schema.
        input: Value,
    },
    /// The outcome of executing a previously requested tool call.
    ToolResult {
        /// The [`ContentBlock::ToolUse`] ID this result answers.
        tool_use_id: ToolId,
        /// Tool output, already flattened to text. Errors are conveyed as
        /// text here rather than as a distinct variant.
        content: String,
    },
}

/// Serialize-only payload for a chat completion request.
///
/// There is no `Deserialize` impl because responses are parsed into
/// provider-specific types instead.
#[allow(dead_code)]
#[derive(Serialize)]
pub struct ApiRequest {
    /// Model to invoke.
    pub model: ModelId,
    /// Upper bound on tokens the model may generate for this request. This
    /// caps the response only, not the combined prompt plus response.
    pub max_tokens: u32,
    /// System prompt sent as a top-level field, separate from
    /// [`ApiRequest::messages`].
    pub system: String,
    /// Conversation history in chronological order.
    pub messages: Vec<Message>,
    /// Tools the model may call. An empty list disables tool use.
    pub tools: Vec<ToolDefinition>,
}

/// Declaration of a tool advertised to the model.
#[derive(Serialize, Clone, Debug)]
pub struct ToolDefinition {
    /// Tool name; must match the [`ContentBlock::ToolUse::name`] the model
    /// emits when calling it.
    pub name: String,
    /// Natural-language description. This is prompt material and directly
    /// affects whether the model selects the tool correctly.
    pub description: String,
    /// JSON Schema describing accepted arguments.
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
    fn content_block_serde_roundtrip() {
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
    fn message_user_string() {
        let msg = Message::user(String::from("Test"));
        assert_eq!(msg.role, "user");
        match &msg.content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "Test"),
            other => panic!("Expected Text content block, got: {other:?}"),
        }
    }

    #[test]
    fn message_tool_results_detailed() {
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
    fn content_block_tool_use_serialization() {
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
    fn content_block_tool_result_serialization() {
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
    fn message_serialization_roundtrip() {
        let original = Message::user("Test message");
        let json = serde_json::to_string(&original).expect("serialize Message");
        let deserialized: Message = serde_json::from_str(&json).expect("deserialize Message");
        assert_eq!(deserialized.role, "user");
        assert_eq!(deserialized.content.len(), 1);
    }

    #[test]
    fn message_with_multiple_content_blocks() {
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
    fn api_request_serialization() {
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
