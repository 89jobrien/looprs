//! Decodes provider SSE frames and assembles streamed text, tool calls, and usage.

use std::collections::BTreeMap;

use looprs_core::ports::{InferenceDelta, InferenceResponse, InferenceStreamEvent, Usage};
use serde_json::Value;

use crate::api::ContentBlock;
use crate::errors::ProviderError;
use crate::types::{ToolId, ToolName};

#[derive(Debug, Default)]
pub(super) struct SseDecoder {
    buffer: Vec<u8>,
}

impl SseDecoder {
    pub(super) fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buffer.extend_from_slice(chunk);
        let mut events = Vec::new();
        while let Some((event_end, delimiter_len)) = event_boundary(&self.buffer) {
            let remaining = self.buffer.split_off(event_end + delimiter_len);
            let event = &self.buffer[..event_end];
            if let Some(data) = parse_event_data(event) {
                events.push(data);
            }
            self.buffer = remaining;
        }
        events
    }
}

fn event_boundary(buffer: &[u8]) -> Option<(usize, usize)> {
    let lf = buffer.windows(2).position(|window| window == b"\n\n");
    let crlf = buffer.windows(4).position(|window| window == b"\r\n\r\n");
    match (lf, crlf) {
        (Some(lf), Some(crlf)) if lf <= crlf => Some((lf, 2)),
        (Some(_), Some(crlf)) => Some((crlf, 4)),
        (Some(lf), None) => Some((lf, 2)),
        (None, Some(crlf)) => Some((crlf, 4)),
        (None, None) => None,
    }
}

fn parse_event_data(event: &[u8]) -> Option<String> {
    let event = std::str::from_utf8(event).ok()?;
    let data = event
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(|value| value.strip_prefix(' ').unwrap_or(value))
        .collect::<Vec<_>>();
    (!data.is_empty()).then(|| data.join("\n"))
}

#[derive(Debug, Default)]
struct PendingOpenAiToolCall {
    id: String,
    name: String,
    arguments: String,
}

#[derive(Debug, Default)]
pub(super) struct OpenAiStreamState {
    text: String,
    tools: BTreeMap<usize, PendingOpenAiToolCall>,
    stop_reason: String,
    usage: Usage,
}

impl OpenAiStreamState {
    pub(super) fn ingest(
        &mut self,
        payload: &Value,
    ) -> Result<Vec<InferenceStreamEvent>, ProviderError> {
        if let Some(error) = payload.get("error") {
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("OpenAI stream returned an error");
            return Err(ProviderError::ApiError(message.to_string()));
        }
        if let Some(usage) = payload.get("usage") {
            self.usage.input_tokens = token_count(usage, "prompt_tokens");
            self.usage.output_tokens = token_count(usage, "completion_tokens");
        }

        let mut events = Vec::new();
        let Some(choice) = payload
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
        else {
            return Ok(events);
        };

        if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
            self.stop_reason = reason.to_string();
        }
        let Some(delta) = choice.get("delta") else {
            return Ok(events);
        };
        if let Some(text) = delta.get("content").and_then(Value::as_str)
            && !text.is_empty()
        {
            self.text.push_str(text);
            events.push(InferenceStreamEvent::Delta(InferenceDelta::Text(
                text.to_string(),
            )));
        }
        if let Some(tool_calls) = delta.get("tool_calls").and_then(Value::as_array) {
            for tool_call in tool_calls {
                let index = tool_call.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                let pending = self.tools.entry(index).or_default();
                let id = tool_call.get("id").and_then(Value::as_str);
                if let Some(id) = id {
                    pending.id.push_str(id);
                }
                let function = tool_call.get("function");
                let name = function
                    .and_then(|value| value.get("name"))
                    .and_then(Value::as_str);
                if let Some(name) = name {
                    pending.name.push_str(name);
                }
                let arguments = function
                    .and_then(|value| value.get("arguments"))
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                pending.arguments.push_str(arguments);
                events.push(InferenceStreamEvent::Delta(InferenceDelta::ToolCall {
                    index,
                    id: id.map(ToOwned::to_owned),
                    name: name.map(ToOwned::to_owned),
                    arguments_fragment: arguments.to_string(),
                }));
            }
        }
        Ok(events)
    }

    pub(super) fn finish(self) -> Result<InferenceResponse, ProviderError> {
        let mut content = Vec::new();
        if !self.text.is_empty() {
            content.push(ContentBlock::Text { text: self.text });
        }
        for (_, tool) in self.tools {
            if tool.id.is_empty() || tool.name.is_empty() {
                return Err(ProviderError::InvalidResponse(
                    "streamed tool call is missing id or name".to_string(),
                ));
            }
            let input = serde_json::from_str(&tool.arguments).map_err(|error| {
                ProviderError::InvalidResponse(format!(
                    "invalid streamed tool arguments for {}: {error}",
                    tool.name
                ))
            })?;
            content.push(ContentBlock::ToolUse {
                id: ToolId::new(tool.id),
                name: ToolName::new(tool.name),
                input,
            });
        }
        Ok(InferenceResponse {
            content,
            stop_reason: if self.stop_reason.is_empty() {
                "stop".to_string()
            } else {
                self.stop_reason
            },
            usage: self.usage,
        })
    }
}

#[derive(Debug)]
enum AnthropicBlock {
    Text(String),
    Tool {
        id: String,
        name: String,
        arguments: String,
        initial_input: Value,
    },
}

#[derive(Debug, Default)]
pub(super) struct AnthropicStreamState {
    blocks: BTreeMap<usize, AnthropicBlock>,
    stop_reason: String,
    usage: Usage,
}

impl AnthropicStreamState {
    pub(super) fn ingest(
        &mut self,
        payload: &Value,
    ) -> Result<Vec<InferenceStreamEvent>, ProviderError> {
        let event_type = payload
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if event_type == "error" {
            let message = payload
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("Anthropic stream returned an error");
            return Err(ProviderError::ApiError(message.to_string()));
        }
        let mut events = Vec::new();
        match event_type {
            "message_start" => {
                if let Some(usage) = payload.pointer("/message/usage") {
                    self.usage.input_tokens = token_count(usage, "input_tokens");
                    self.usage.output_tokens = token_count(usage, "output_tokens");
                }
            }
            "content_block_start" => {
                let index = stream_index(payload)?;
                let block = payload.get("content_block").ok_or_else(|| {
                    ProviderError::InvalidResponse(
                        "content_block_start missing content_block".to_string(),
                    )
                })?;
                match block.get("type").and_then(Value::as_str) {
                    Some("text") => {
                        let text = block
                            .get("text")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string();
                        if !text.is_empty() {
                            events.push(InferenceStreamEvent::Delta(InferenceDelta::Text(
                                text.clone(),
                            )));
                        }
                        self.blocks.insert(index, AnthropicBlock::Text(text));
                    }
                    Some("tool_use") => {
                        let id = required_string(block, "id")?;
                        let name = required_string(block, "name")?;
                        self.blocks.insert(
                            index,
                            AnthropicBlock::Tool {
                                id: id.clone(),
                                name: name.clone(),
                                arguments: String::new(),
                                initial_input: block.get("input").cloned().unwrap_or(Value::Null),
                            },
                        );
                        events.push(InferenceStreamEvent::Delta(InferenceDelta::ToolCall {
                            index,
                            id: Some(id),
                            name: Some(name),
                            arguments_fragment: String::new(),
                        }));
                    }
                    _ => {}
                }
            }
            "content_block_delta" => {
                let index = stream_index(payload)?;
                let delta = payload.get("delta").ok_or_else(|| {
                    ProviderError::InvalidResponse("content delta missing delta".to_string())
                })?;
                match delta.get("type").and_then(Value::as_str) {
                    Some("text_delta") => {
                        let text = delta
                            .get("text")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        let Some(AnthropicBlock::Text(accumulated)) = self.blocks.get_mut(&index)
                        else {
                            return Err(ProviderError::InvalidResponse(
                                "text delta references an unknown block".to_string(),
                            ));
                        };
                        accumulated.push_str(text);
                        if !text.is_empty() {
                            events.push(InferenceStreamEvent::Delta(InferenceDelta::Text(
                                text.to_string(),
                            )));
                        }
                    }
                    Some("input_json_delta") => {
                        let fragment = delta
                            .get("partial_json")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        let Some(AnthropicBlock::Tool { arguments, .. }) =
                            self.blocks.get_mut(&index)
                        else {
                            return Err(ProviderError::InvalidResponse(
                                "tool delta references an unknown block".to_string(),
                            ));
                        };
                        arguments.push_str(fragment);
                        events.push(InferenceStreamEvent::Delta(InferenceDelta::ToolCall {
                            index,
                            id: None,
                            name: None,
                            arguments_fragment: fragment.to_string(),
                        }));
                    }
                    _ => {}
                }
            }
            "message_delta" => {
                if let Some(reason) = payload
                    .pointer("/delta/stop_reason")
                    .and_then(Value::as_str)
                {
                    self.stop_reason = reason.to_string();
                }
                if let Some(usage) = payload.get("usage") {
                    self.usage.output_tokens = token_count(usage, "output_tokens");
                }
            }
            _ => {}
        }
        Ok(events)
    }

    pub(super) fn finish(self) -> Result<InferenceResponse, ProviderError> {
        let mut content = Vec::new();
        for (_, block) in self.blocks {
            match block {
                AnthropicBlock::Text(text) if !text.is_empty() => {
                    content.push(ContentBlock::Text { text });
                }
                AnthropicBlock::Text(_) => {}
                AnthropicBlock::Tool {
                    id,
                    name,
                    arguments,
                    initial_input,
                } => {
                    let input = if arguments.is_empty() {
                        initial_input
                    } else {
                        serde_json::from_str(&arguments).map_err(|error| {
                            ProviderError::InvalidResponse(format!(
                                "invalid streamed tool arguments for {name}: {error}"
                            ))
                        })?
                    };
                    content.push(ContentBlock::ToolUse {
                        id: ToolId::new(id),
                        name: ToolName::new(name),
                        input,
                    });
                }
            }
        }
        Ok(InferenceResponse {
            content,
            stop_reason: if self.stop_reason.is_empty() {
                "end_turn".to_string()
            } else {
                self.stop_reason
            },
            usage: self.usage,
        })
    }
}

fn token_count(value: &Value, field: &str) -> u32 {
    value
        .get(field)
        .and_then(Value::as_u64)
        .unwrap_or_default()
        .min(u64::from(u32::MAX)) as u32
}

fn stream_index(payload: &Value) -> Result<usize, ProviderError> {
    payload
        .get("index")
        .and_then(Value::as_u64)
        .map(|index| index as usize)
        .ok_or_else(|| ProviderError::InvalidResponse("stream event missing index".to_string()))
}

fn required_string(value: &Value, field: &str) -> Result<String, ProviderError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            ProviderError::InvalidResponse(format!("streamed tool call missing {field}"))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sse_decoder_preserves_fragmented_frames() {
        let frame = b"data: {\"value\":\"fragmented\"}\n\n";
        for split in 0..=frame.len() {
            let mut decoder = SseDecoder::default();
            let mut events = decoder.push(&frame[..split]);
            events.extend(decoder.push(&frame[split..]));
            assert_eq!(events, vec!["{\"value\":\"fragmented\"}"]);
        }
    }

    #[test]
    fn openai_stream_builds_text_tool_usage_and_stop_reason() {
        let mut state = OpenAiStreamState::default();
        state
            .ingest(&json!({"choices":[{"delta":{"content":"hi "}}]}))
            .unwrap();
        state.ingest(&json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"read","arguments":"{\"path\":"}}]}}]})).unwrap();
        state.ingest(&json!({"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"README.md\"}"}}]},"finish_reason":"tool_calls"}]})).unwrap();
        state
            .ingest(&json!({"choices":[],"usage":{"prompt_tokens":4,"completion_tokens":5}}))
            .unwrap();

        let response = state.finish().unwrap();
        assert_eq!(response.stop_reason, "tool_calls");
        assert_eq!(response.usage.input_tokens, 4);
        assert_eq!(response.usage.output_tokens, 5);
        assert!(matches!(&response.content[0], ContentBlock::Text { text } if text == "hi "));
        assert!(
            matches!(&response.content[1], ContentBlock::ToolUse { name, input, .. }
            if name.as_str() == "read" && input["path"] == "README.md")
        );
    }

    #[test]
    fn openai_stream_rejects_malformed_tool_arguments() {
        let mut state = OpenAiStreamState::default();
        state.ingest(&json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"read","arguments":"{"}}]}}]})).unwrap();
        assert!(state.finish().is_err());
    }

    #[test]
    fn provider_stream_errors_fail_closed() {
        assert!(
            OpenAiStreamState::default()
                .ingest(&json!({"error":{"message":"rate limited"}}))
                .is_err()
        );
        assert!(
            AnthropicStreamState::default()
                .ingest(&json!({"type":"error","error":{"message":"overloaded"}}))
                .is_err()
        );
    }

    #[test]
    fn anthropic_stream_builds_tool_call_and_usage() {
        let mut state = AnthropicStreamState::default();
        state.ingest(&json!({"type":"message_start","message":{"usage":{"input_tokens":3,"output_tokens":0}}})).unwrap();
        state.ingest(&json!({"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"tool_1","name":"grep","input":{}}})).unwrap();
        state.ingest(&json!({"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"pattern\":\"x\"}"}})).unwrap();
        state.ingest(&json!({"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":2}})).unwrap();

        let response = state.finish().unwrap();
        assert_eq!(response.usage.input_tokens, 3);
        assert_eq!(response.usage.output_tokens, 2);
        assert!(
            matches!(&response.content[0], ContentBlock::ToolUse { name, input, .. }
            if name.as_str() == "grep" && input["pattern"] == "x")
        );
    }
}
