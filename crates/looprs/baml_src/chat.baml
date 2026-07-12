/// A single turn in a multi-turn conversation.
class ChatMessage {
    role string @description("'user' or 'assistant'")
    content string @description("Text content of the message")
}

/// Perform a multi-turn chat inference.
///
/// `system` is injected as the system prompt.
/// `messages` is the full conversation history up to and including the
/// current user turn.
function Chat(system: string, messages: ChatMessage[]) -> string {
    client DefaultClient
    prompt #"
        {{ _.role("system") }}
        {{ system }}

        {% for msg in messages %}
        {{ _.role(msg.role) }}
        {{ msg.content }}
        {% endfor %}
    "#
}
