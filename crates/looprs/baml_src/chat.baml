/// A single turn in a multi-turn conversation.
class ChatMessage {
    role string @description("'user' or 'assistant'")
    content string @description("Text content of the message")
}

/// Describes a tool the model may call. `input_schema` is the tool's JSON
/// schema serialized as a string (kept as a string to avoid modeling
/// arbitrary JSON Schema shapes in BAML).
class ToolDefinition {
    name string @description("Tool name")
    description string @description("What the tool does")
    input_schema string @description("JSON schema for the tool's input, serialized as a string")
}

/// A single tool invocation requested by the model.
class ToolCall {
    name string @description("Name of the tool to call")
    arguments string @description("JSON-encoded arguments for the tool call")
    id string @description("Unique identifier for this tool call")
}

/// Perform a multi-turn chat inference, optionally offering tools.
///
/// `system` is injected as the system prompt.
/// `messages` is the full conversation history up to and including the
/// current user turn.
/// `tools` is the list of tools the model may call; pass an empty list when
/// no tools are available.
///
/// Returns either plain text, or a list of tool calls when the model elects
/// to use a tool.
function Chat(system: string, messages: ChatMessage[], tools: ToolDefinition[]) -> string | ToolCall[] {
    client DefaultClient
    prompt #"
        {{ _.role("system") }}
        {{ system }}

        {% if tools|length > 0 %}
        You have access to the following tools:
        {% for tool in tools %}
        - {{ tool.name }}: {{ tool.description }}
          input schema: {{ tool.input_schema }}
        {% endfor %}

        If you need to call one or more tools to respond, reply with ONLY a
        JSON array of tool calls, each shaped like:
        {"name": "<tool name>", "arguments": "<JSON-encoded arguments string>", "id": "<unique id>"}

        Otherwise, reply with plain text as normal.
        {% endif %}

        {% for msg in messages %}
        {{ _.role(msg.role) }}
        {{ msg.content }}
        {% endfor %}
    "#
}
