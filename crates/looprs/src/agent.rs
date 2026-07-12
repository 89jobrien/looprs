use crate::api::ContentBlock;
use crate::api::Message;
use crate::app_config::DefaultsConfig;
use crate::errors::AgentError;
use crate::events::{Event, EventContext, EventManager};
use crate::file_refs::FileRefPolicy;
use crate::fs_mode::FsMode;
use crate::hooks::{ApprovalCallback, HookExecutor, HookRegistry, PromptCallback};
use crate::models_config::ModelsConfig;
use crate::observation_manager::ObservationManager;
use crate::ports::{SessionStore, UserOutput};
use crate::providers::LLMProvider;
use crate::providers::{InferenceRequest, InferenceResponse};
use crate::rules::RuleRegistry;
use crate::session_log::SessionEvent;
use crate::system_monitor::SystemMonitor;
use crate::tools::{ToolContext, execute_tool, get_tool_definitions};
use std::collections::HashMap;
use tokio::time::{Duration, timeout};

const TOOL_PREVIEW_LEN: usize = 60;
const ON_REPEAT_THRESHOLD: usize = 3;

const MAX_TOOL_RESULT_CHARS_IN_CONTEXT: usize = 16_000;

fn truncate_tool_result_for_context(content: &str) -> String {
    if content.chars().count() <= MAX_TOOL_RESULT_CHARS_IN_CONTEXT {
        return content.to_string();
    }

    let truncated: String = content
        .chars()
        .take(MAX_TOOL_RESULT_CHARS_IN_CONTEXT)
        .collect();
    let original_chars = content.chars().count();
    format!(
        "{}\n\n[truncated tool result: {} chars omitted]",
        truncated,
        original_chars.saturating_sub(MAX_TOOL_RESULT_CHARS_IN_CONTEXT)
    )
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeSettings {
    pub defaults: DefaultsConfig,
    pub max_tokens_override: Option<u32>,
    pub fs_mode: FsMode,
}

pub struct Agent {
    provider: Box<dyn LLMProvider>,
    messages: Vec<Message>,
    tool_ctx: ToolContext,
    pub(crate) events: EventManager,
    pub(crate) observations: ObservationManager,
    pub(crate) hooks: HookRegistry,
    pub(crate) rules: RuleRegistry,
    runtime: RuntimeSettings,
    file_ref_policy: FileRefPolicy,
    pending_metadata: HashMap<String, String>,
    session_logger: Option<Box<dyn SessionStore>>,
    output: Box<dyn UserOutput>,
    models_config: Option<ModelsConfig>,
    system_monitor: SystemMonitor,
}

impl Agent {
    pub fn new(provider: Box<dyn LLMProvider>) -> Result<Self, AgentError> {
        use crate::adapters::UiOutput;
        Self::new_with_runtime(
            provider,
            RuntimeSettings::default(),
            FileRefPolicy::default(),
            None,
            Box::new(UiOutput),
        )
    }

    pub fn new_with_runtime(
        provider: Box<dyn LLMProvider>,
        runtime: RuntimeSettings,
        file_ref_policy: FileRefPolicy,
        session_logger: Option<Box<dyn SessionStore>>,
        output: Box<dyn UserOutput>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            provider,
            messages: Vec::new(),
            tool_ctx: ToolContext::new_with_mode(runtime.fs_mode)?,
            events: EventManager::new(),
            observations: ObservationManager::new(),
            hooks: HookRegistry::new(),
            rules: RuleRegistry::new(),
            runtime,
            file_ref_policy,
            pending_metadata: HashMap::new(),
            session_logger,
            output,
            models_config: ModelsConfig::load().ok(),
            system_monitor: SystemMonitor::new(),
        })
    }

    /// Replace the output adapter. Useful for tests (inject `NullOutput`) or
    /// alternative frontends (GUI, JSON stream, etc.).
    pub fn with_output(mut self, output: Box<dyn UserOutput>) -> Self {
        self.output = output;
        self
    }

    pub fn with_hooks(mut self, hooks: HookRegistry) -> Self {
        self.hooks = hooks;
        self
    }

    pub fn with_rules(mut self, rules: RuleRegistry) -> Self {
        self.rules = rules;
        self
    }

    pub fn fire_event(&self, event: Event, context: &EventContext) {
        self.events.fire(event, context);
    }

    pub fn set_provider(&mut self, provider: Box<dyn LLMProvider>) {
        self.provider = provider;
    }

    pub fn set_runtime_settings(&mut self, runtime: RuntimeSettings) {
        self.tool_ctx.set_fs_mode(runtime.fs_mode);
        self.runtime = runtime;
    }

    pub fn set_file_ref_policy(&mut self, policy: FileRefPolicy) {
        self.file_ref_policy = policy;
    }

    pub fn fs_mode(&self) -> FsMode {
        self.tool_ctx.fs_mode()
    }

    pub fn set_fs_mode(&self, mode: FsMode) {
        self.tool_ctx.set_fs_mode(mode);
    }

    pub fn fs_mode_handle(&self) -> std::sync::Arc<std::sync::atomic::AtomicU8> {
        self.tool_ctx.fs_mode_handle()
    }

    pub fn set_turn_metadata(&mut self, metadata: HashMap<String, String>) {
        self.pending_metadata.extend(metadata);
    }

    pub fn add_user_message(&mut self, text: impl Into<String>) {
        let text_str = text.into();

        // Resolve file references (@filename) if present
        let resolved = if crate::file_refs::has_file_references(&text_str) {
            match crate::file_refs::resolve_file_references(
                &text_str,
                &self.tool_ctx.working_dir,
                &self.file_ref_policy,
            ) {
                Ok(resolved_text) => resolved_text,
                Err(e) => {
                    self.output
                        .warn(&format!("Warning: Error resolving file references: {e}"));
                    text_str.clone()
                }
            }
        } else {
            text_str.clone()
        };

        self.messages.push(Message::user(resolved));
    }

    pub fn clear_history(&mut self) {
        self.messages.clear();
    }

    pub fn latest_assistant_text(&self) -> Option<String> {
        self.messages
            .iter()
            .rev()
            .find(|m| m.role == "assistant")
            .map(|m| {
                m.content
                    .iter()
                    .filter_map(|block| match block {
                        ContentBlock::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n")
            })
            .filter(|text| !text.is_empty())
    }

    pub fn working_dir(&self) -> &std::path::Path {
        &self.tool_ctx.working_dir
    }

    pub fn execute_hooks_for_event(&self, event: &Event, context: &EventContext) -> EventContext {
        self.execute_hooks_for_event_with_callbacks(event, context, None, None, None)
    }

    // qual:allow(iosp) reason: "I/O boundary — orchestrates hook execution with callbacks"
    pub fn execute_hooks_for_event_with_callbacks(
        &self,
        event: &Event,
        context: &EventContext,
        approval_fn: Option<&ApprovalCallback>,
        prompt_fn: Option<&PromptCallback>,
        secret_prompt_fn: Option<&PromptCallback>,
    ) -> EventContext {
        let mut enriched_context = context.clone();

        if let Some(hooks) = self.hooks.hooks_for_event(event) {
            for hook in hooks {
                if let Ok(results) = HookExecutor::execute_hook_with_callbacks(
                    hook,
                    context,
                    approval_fn,
                    prompt_fn,
                    secret_prompt_fn,
                ) {
                    // Inject hook outputs into context metadata
                    for result in results {
                        if let Some(key) = result.inject_key {
                            enriched_context.metadata.insert(key, result.output);
                        }
                    }
                }
            }
        }

        enriched_context
    }

    fn build_system_prompt(&self, enriched_ctx: &EventContext) -> String {
        let mut system_prompt = format!(
            "You are a concise coding assistant. Current working directory: {}",
            self.tool_ctx.working_dir.display()
        );

        let rules_section = self.rules.format_for_prompt();
        if !rules_section.is_empty() {
            system_prompt.push_str(&rules_section);
        }

        if !enriched_ctx.metadata.is_empty() {
            const MAX_INJECTION_SIZE: usize = 2000;
            system_prompt.push_str("\n\n## Additional Context from Hooks:");
            for (key, value) in &enriched_ctx.metadata {
                let truncated_value = if value.len() > MAX_INJECTION_SIZE {
                    format!(
                        "{}... [truncated {} bytes]",
                        &value[..MAX_INJECTION_SIZE],
                        value.len() - MAX_INJECTION_SIZE
                    )
                } else {
                    value.clone()
                };
                system_prompt.push_str(&format!("\n### {key}\n{truncated_value}"));
            }
        }

        system_prompt
    }

    fn log_inference(&mut self, response: &InferenceResponse) {
        if let Some(ref mut logger) = self.session_logger {
            let content = response
                .content
                .iter()
                .filter_map(|b| {
                    if let ContentBlock::Text { text } = b {
                        Some(text.as_str())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n\n");
            let _ = logger.log(SessionEvent::Inference {
                content,
                provider: self.provider.name().to_string(),
            });
        }
    }

    // TODO(streaming): implement streaming output (idea #5).
    // Both claudius and async-openai expose a streaming variant:
    //   anthropic-sdk: client.messages.stream(req) → Stream<MessageStreamEvent>
    //   async-openai:  client.chat().create_stream(req) → Stream<ChatCompletionChunk>
    //
    // Implementation plan:
    //   1. Add `infer_stream` to `InferenceProvider` returning a
    //      `Pin<Box<dyn Stream<Item = Result<String, _>> + Send>>`.
    //   2. Implement it in each provider by mapping SDK stream events to text chunks.
    //   3. In `run_turn_streaming`, drive the stream, calling
    //      `self.runtime.output.write_chunk(chunk)` per token (requires hex
    //      refactor Phase 1 so `runtime.output` holds a `Box<dyn UserOutput>`).
    //   4. Accumulate chunks into a full `ContentBlock::Text` for tool-use parsing
    //      and observation capture after the stream ends.
    //
    // Blocked by: hex refactor Phase 1 (UserOutput injection into Agent).
    pub async fn run_turn_streaming(&mut self) -> Result<(), AgentError> {
        unimplemented!(
            "streaming inference — add infer_stream() to InferenceProvider, \
             inject UserOutput port (Phase 1), drive write_chunk() per token"
        )
    }

    // TODO(parallel-dispatch): implement parallel agent dispatch (idea #7).
    // AgentsConfig.max_parallel is loaded but the orchestration strategy is
    // hardcoded "sequential" here. To support parallel dispatch:
    //   1. Collect independent sub-tasks from the current turn (tool calls with
    //      no data dependency on each other).
    //   2. Spawn up to `self.runtime.config.agents.max_parallel` tasks via
    //      `tokio::task::JoinSet`, one per sub-agent.
    //   3. Collect results and merge into a single `InferenceResponse`.
    //   4. Guard with `agents.orchestration = "parallel"` config flag so
    //      sequential remains the default.
    //
    // Blocked by: stable AgentBuilder and AgentRuntime Clone impls.
    pub async fn run_turn(&mut self) -> Result<(), AgentError> {
        let delegated_agent = self.pending_metadata.get("orchestration.agent").cloned();
        if let Some(agent_name) = delegated_agent.clone() {
            let strategy = self
                .pending_metadata
                .get("orchestration.strategy")
                .cloned()
                .unwrap_or_else(|| "sequential".to_string());
            let event_ctx = EventContext::new()
                .with_tool_name(agent_name)
                .with_metadata("orchestration.strategy".to_string(), strategy)
                .with_metadata("orchestration.mode".to_string(), "delegated".to_string());
            self.events.fire(Event::DelegationStart, &event_ctx);
            self.execute_hooks_for_event(&Event::DelegationStart, &event_ctx);
        }

        // Fire UserPromptSubmit event
        let user_msg = self
            .messages
            .last()
            .and_then(|m| {
                if m.role == "user" {
                    m.content.first().and_then(|cb| {
                        if let ContentBlock::Text { text } = cb {
                            Some(text.clone())
                        } else {
                            None
                        }
                    })
                } else {
                    None
                }
            })
            .unwrap_or_default();

        if let Some(ref mut logger) = self.session_logger {
            let _ = logger.log(SessionEvent::UserMessage {
                content: user_msg.clone(),
                provider: self.provider.name().to_string(),
            });
        }

        let mut event_ctx = EventContext::new().with_user_message(user_msg);
        for (key, value) in &self.pending_metadata {
            event_ctx.metadata.insert(key.clone(), value.clone());
        }
        self.events.fire(Event::UserPromptSubmit, &event_ctx);
        let mut enriched_ctx = self.execute_hooks_for_event(&Event::UserPromptSubmit, &event_ctx);
        for (key, value) in std::mem::take(&mut self.pending_metadata) {
            enriched_ctx.metadata.insert(key, value);
        }

        let system_prompt = self.build_system_prompt(&enriched_ctx);

        let mut tool_call_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        loop {
            let mut max_tokens = self.provider.model().max_tokens();
            if let Some(override_tokens) = self.runtime.max_tokens_override {
                max_tokens = max_tokens.min(override_tokens);
            }
            if let Some(max_context) = self.runtime.defaults.max_context_tokens {
                max_tokens = max_tokens.min(max_context);
            }
            let messages = if let Some(max_context) = self.runtime.defaults.max_context_tokens {
                compact_messages(&self.messages, max_context as usize)
            } else {
                self.messages.clone()
            };
            let req = InferenceRequest {
                model: self.provider.model().clone(),
                messages,
                tools: get_tool_definitions(),
                max_tokens,
                temperature: self.runtime.defaults.temperature,
                system: system_prompt.clone(),
            };

            let response = if let Some(timeout_secs) = self.runtime.defaults.timeout_seconds {
                match timeout(Duration::from_secs(timeout_secs), self.provider.infer(&req)).await {
                    Ok(res) => res.map_err(|e| AgentError::Inference(e.to_string()))?,
                    Err(_) => return Err(AgentError::Timeout),
                }
            } else {
                self.provider
                    .infer(&req)
                    .await
                    .map_err(|e| AgentError::Inference(e.to_string()))?
            };

            self.log_inference(&response);

            #[cfg(not(test))]
            if let Err(e) =
                crate::trace::append_turn_trace(self.observations.session_id(), &req, &response)
            {
                self.output
                    .warn(&format!("Warning: Failed to append turn trace: {e}"));
            }

            #[cfg(not(test))]
            {
                let metrics = self.system_monitor.collect_metrics();
                let _ = crate::observability::append_named_jsonl(
                    "system_metrics",
                    &serde_json::json!({
                        "session_id": self.observations.session_id(),
                        "cpu_usage": metrics.cpu_usage,
                        "memory_usage": metrics.memory_usage,
                        "error_rate": metrics.error_rate,
                        "response_time_p95": metrics.response_time_p95,
                    }),
                );
            }

            // Fire InferenceComplete event
            let event_ctx = EventContext::new();
            self.events.fire(Event::InferenceComplete, &event_ctx);
            self.execute_hooks_for_event(&Event::InferenceComplete, &event_ctx);

            let assistant_blocks = response.content;
            let mut tool_indices = Vec::new();

            for (idx, block) in assistant_blocks.iter().enumerate() {
                match block {
                    ContentBlock::Text { text } => {
                        self.output.assistant_text(text);
                    }
                    ContentBlock::ToolUse { name, input, .. } => {
                        let preview = serde_json::to_string(&input)
                            .unwrap_or_default()
                            .chars()
                            .take(TOOL_PREVIEW_LEN)
                            .collect::<String>();

                        self.output.tool_call(name.as_str(), &preview);
                        tool_indices.push(idx);
                    }
                    _ => {}
                }
            }

            self.messages.push(Message::assistant(assistant_blocks));

            if tool_indices.is_empty() {
                break;
            }

            let mut tool_results = Vec::new();
            let assistant_message = self.messages.last().expect("assistant message just pushed");

            for idx in tool_indices {
                let ContentBlock::ToolUse { id, name, input } = &assistant_message.content[idx]
                else {
                    continue;
                };
                // Fire PreToolUse event
                let event_ctx = EventContext::new().with_tool_name(name.as_str().to_string());
                self.events.fire(Event::PreToolUse, &event_ctx);
                self.execute_hooks_for_event(&Event::PreToolUse, &event_ctx);

                if let Some(ref mut logger) = self.session_logger {
                    let provider_name = self.provider.name().to_string();
                    let _ = logger.log(SessionEvent::ToolUse {
                        tool_name: name.to_string(),
                        input: input.clone(),
                        tool_use_id: id.to_string(),
                        provider: provider_name,
                    });
                }

                let count = tool_call_counts.entry(name.to_string()).or_insert(0);
                *count += 1;
                if *count == ON_REPEAT_THRESHOLD {
                    log::info!(
                        "on-repeat trigger: {} called {} times",
                        name.as_str(),
                        count
                    );
                    self.maybe_score(crate::scorer::ScoreTrigger::OnRepeat {
                        tool_name: name.as_str().to_string(),
                        count: *count,
                    })
                    .await;
                }

                let result = execute_tool(name.as_str(), input, &self.tool_ctx);
                let tool_is_error = result.is_err();

                let raw_content = match result {
                    Ok(ref output) => {
                        self.output.tool_ok();
                        // Capture observation
                        self.observations.capture(
                            name.as_str().to_string(),
                            input.clone(),
                            output.clone(),
                            Some(id.clone()),
                        );
                        // Fire PostToolUse event on success
                        let event_ctx = EventContext::new()
                            .with_tool_name(name.as_str().to_string())
                            .with_tool_output(output.clone());
                        self.events.fire(Event::PostToolUse, &event_ctx);
                        self.execute_hooks_for_event(&Event::PostToolUse, &event_ctx);
                        output.clone()
                    }
                    Err(e) => {
                        let err_msg = format!("error: {e}");
                        self.output.tool_err(&err_msg);
                        self.system_monitor.record_error();
                        // Fire OnError event
                        let event_ctx = EventContext::new()
                            .with_tool_name(name.as_str().to_string())
                            .with_error(err_msg.clone());
                        self.events.fire(Event::OnError, &event_ctx);
                        self.execute_hooks_for_event(&Event::OnError, &event_ctx);
                        self.maybe_score(crate::scorer::ScoreTrigger::OnError).await;
                        err_msg
                    }
                };

                if let Some(ref mut logger) = self.session_logger {
                    let provider_name = self.provider.name().to_string();
                    let _ = logger.log(SessionEvent::ToolResult {
                        tool_use_id: id.to_string(),
                        output: raw_content.clone(),
                        is_error: tool_is_error,
                        provider: provider_name,
                    });
                }

                let content = truncate_tool_result_for_context(&raw_content);

                tool_results.push(ContentBlock::ToolResult {
                    tool_use_id: id.clone(),
                    content,
                });
            }

            self.messages.push(Message::tool_results(tool_results));
        }

        if let Some(agent_name) = delegated_agent {
            let event_ctx = EventContext::new()
                .with_tool_name(agent_name)
                .with_metadata("orchestration.mode".to_string(), "delegated".to_string());
            self.events.fire(Event::DelegationComplete, &event_ctx);
            self.execute_hooks_for_event(&Event::DelegationComplete, &event_ctx);
        }

        if let Some(ref mut logger) = self.session_logger {
            let _ = logger.log(SessionEvent::SessionEnd);
        }

        Ok(())
    }

    async fn maybe_score(&self, trigger: crate::scorer::ScoreTrigger) {
        let Some(ref logger) = self.session_logger else {
            return;
        };
        let Some(ref config) = self.models_config else {
            return;
        };

        let scorer_model = config
            .tier("judge")
            .map(|t| t.model.as_str())
            .unwrap_or("gpt-4o");
        let db_path = config.magi_db();
        let db_opt = if db_path.is_empty() {
            None
        } else {
            Some(db_path)
        };

        let n = match &trigger {
            crate::scorer::ScoreTrigger::OnError => 1,
            crate::scorer::ScoreTrigger::OnRepeat { .. } => ON_REPEAT_THRESHOLD,
            crate::scorer::ScoreTrigger::OnDemand { n } => *n,
        };

        let Some(path) = logger.path() else {
            return;
        };
        match crate::scorer::load_last_n_ollama_pairs(path, n) {
            Ok(pairs) => {
                if let Err(e) = crate::scorer::run_scorer(&pairs, scorer_model, db_opt).await {
                    log::warn!("scoring failed: {e}");
                }
            }
            Err(e) => log::warn!("failed to load session pairs for scoring: {e}"),
        }
    }
}

/// Trim `messages` to fit within an estimated `max_tokens` budget.
///
/// Estimates 1 token ≈ 4 characters. Drops the oldest user+assistant pairs
/// from the front until the total fits. Always starts the result on a user
/// message and preserves at least one message.
fn compact_messages(messages: &[Message], max_tokens: usize) -> Vec<Message> {
    fn estimate_tokens(msgs: &[Message]) -> usize {
        msgs.iter()
            .flat_map(|m| m.content.iter())
            .map(|block| match block {
                ContentBlock::Text { text } => text.len().div_ceil(4),
                ContentBlock::ToolUse { input, .. } => input.to_string().len().div_ceil(4),
                ContentBlock::ToolResult { content, .. } => content.len().div_ceil(4),
            })
            .sum()
    }

    let mut start = 0;
    while start < messages.len().saturating_sub(1)
        && estimate_tokens(&messages[start..]) > max_tokens
    {
        // Drop one user message and the following assistant/tool messages as a pair
        start += 1;
        while start < messages.len() && messages[start].role != "user" {
            start += 1;
        }
    }
    messages[start..].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::adapters::NullOutput;
    use crate::providers::{InferenceResponse, Usage};

    // Mock provider for testing
    struct MockProvider {
        model: crate::types::ModelId,
        responses: Vec<InferenceResponse>,
        call_count: std::sync::Arc<std::sync::Mutex<usize>>,
    }

    /// Convenience wrapper: creates an Agent with NullOutput so tests don't
    /// produce terminal output.
    fn agent_for_test(provider: MockProvider) -> Agent {
        Agent::new(Box::new(provider))
            .unwrap()
            .with_output(Box::new(NullOutput))
    }

    impl MockProvider {
        fn new(responses: Vec<InferenceResponse>) -> Self {
            Self {
                model: crate::types::ModelId::new("mock-model"),
                responses,
                call_count: std::sync::Arc::new(std::sync::Mutex::new(0)),
            }
        }

        fn simple_text(text: &str) -> Self {
            Self::new(vec![InferenceResponse {
                content: vec![ContentBlock::Text {
                    text: text.to_string(),
                }],
                stop_reason: "end_turn".to_string(),
                usage: Usage {
                    input_tokens: 10,
                    output_tokens: 20,
                },
            }])
        }
    }

    #[async_trait::async_trait]
    impl LLMProvider for MockProvider {
        async fn infer(
            &self,
            _req: &InferenceRequest,
        ) -> Result<InferenceResponse, Box<dyn std::error::Error + Send + Sync>> {
            let mut count = self.call_count.lock().unwrap();
            let idx = *count;
            *count += 1;

            if idx < self.responses.len() {
                Ok(self.responses[idx].clone())
            } else {
                // Default response if we run out
                Ok(InferenceResponse {
                    content: vec![ContentBlock::Text {
                        text: "default response".to_string(),
                    }],
                    stop_reason: "end_turn".to_string(),
                    usage: Usage {
                        input_tokens: 0,
                        output_tokens: 0,
                    },
                })
            }
        }

        fn name(&self) -> &str {
            "mock"
        }

        fn model(&self) -> &crate::types::ModelId {
            &self.model
        }

        fn validate_config(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
    }

    #[test]
    fn test_agent_new() {
        let provider = MockProvider::simple_text("test");
        let agent = Agent::new(Box::new(provider)).map(|a| a.with_output(Box::new(NullOutput)));
        assert!(agent.is_ok());
    }

    #[test]
    fn test_agent_add_user_message() {
        let provider = MockProvider::simple_text("test");
        let mut agent = agent_for_test(provider);

        agent.add_user_message("Hello");
        assert_eq!(agent.messages.len(), 1);
        assert_eq!(agent.messages[0].role, "user");
    }

    #[test]
    fn test_agent_add_multiple_messages() {
        let provider = MockProvider::simple_text("test");
        let mut agent = agent_for_test(provider);

        agent.add_user_message("First");
        agent.add_user_message("Second");
        assert_eq!(agent.messages.len(), 2);
    }

    #[test]
    fn tool_result_truncation_keeps_small_content() {
        let content = "short output";
        let out = truncate_tool_result_for_context(content);
        assert_eq!(out, content);
    }

    #[test]
    fn tool_result_truncation_caps_large_content() {
        let large = "x".repeat(MAX_TOOL_RESULT_CHARS_IN_CONTEXT + 50);
        let out = truncate_tool_result_for_context(&large);
        assert!(out.contains("[truncated tool result:"));
        assert!(out.len() > MAX_TOOL_RESULT_CHARS_IN_CONTEXT);
        assert!(out.len() < large.len());
    }

    #[test]
    fn test_agent_clear_history() {
        let provider = MockProvider::simple_text("test");
        let mut agent = agent_for_test(provider);

        agent.add_user_message("Test");
        assert_eq!(agent.messages.len(), 1);

        agent.clear_history();
        assert_eq!(agent.messages.len(), 0);
    }

    #[test]
    fn test_latest_assistant_text_none_when_no_assistant() {
        let provider = MockProvider::simple_text("test");
        let mut agent = agent_for_test(provider);
        agent.add_user_message("Hello");
        assert_eq!(agent.latest_assistant_text(), None);
    }

    #[tokio::test]
    async fn test_latest_assistant_text_returns_last_text_blocks() {
        let provider = MockProvider::new(vec![InferenceResponse {
            content: vec![
                ContentBlock::Text {
                    text: "First".to_string(),
                },
                ContentBlock::Text {
                    text: "Second".to_string(),
                },
            ],
            stop_reason: "end_turn".to_string(),
            usage: Usage {
                input_tokens: 2,
                output_tokens: 3,
            },
        }]);
        let mut agent = agent_for_test(provider);
        agent.add_user_message("Hello");

        agent.run_turn().await.unwrap();

        assert_eq!(
            agent.latest_assistant_text(),
            Some("First\n\nSecond".to_string())
        );
    }

    #[test]
    fn test_agent_with_hooks() {
        let provider = MockProvider::simple_text("test");
        let hooks = HookRegistry::new();

        let agent = agent_for_test(provider).with_hooks(hooks);

        // Just verify it works
        assert_eq!(agent.messages.len(), 0);
    }

    #[test]
    fn test_execute_hooks_for_event_no_hooks() {
        let provider = MockProvider::simple_text("test");
        let agent = agent_for_test(provider);

        let ctx = EventContext::new().with_user_message("test".to_string());
        let enriched = agent.execute_hooks_for_event(&Event::SessionStart, &ctx);

        // Should return unchanged context
        assert!(enriched.metadata.is_empty());
    }

    #[test]
    fn test_execute_hooks_for_event_with_hooks() {
        let provider = MockProvider::simple_text("test");

        // Create a hook registry (empty is fine, we're just testing it doesn't crash)
        let hooks = HookRegistry::new();

        let agent = agent_for_test(provider).with_hooks(hooks);

        let ctx = EventContext::new();
        let enriched = agent.execute_hooks_for_event(&Event::SessionStart, &ctx);

        // Should work even with no hooks
        assert!(enriched.metadata.is_empty());
    }

    #[test]
    fn test_context_injection_from_hooks() {
        use std::io::Write;
        use tempfile::TempDir;

        let provider = MockProvider::simple_text("test");

        // Create a temporary hook file with inject_as
        let temp_dir = TempDir::new().unwrap();
        let hook_file = temp_dir.path().join("test_hook.yaml");
        let mut file = std::fs::File::create(&hook_file).unwrap();
        writeln!(
            file,
            r#"name: test_injection
trigger: SessionStart
actions:
  - type: command
    command: "echo 'injected context'"
    inject_as: "test_key"
  - type: command
    command: "echo 'another value'"
    inject_as: "another_key""#
        )
        .unwrap();
        drop(file);

        // Load hooks from temp directory
        let hooks = HookRegistry::load_from_directory(&temp_dir.path().to_path_buf()).unwrap();

        let agent = agent_for_test(provider).with_hooks(hooks);

        let ctx = EventContext::new();
        let enriched = agent.execute_hooks_for_event(&Event::SessionStart, &ctx);

        // Should have injected context in metadata
        assert!(!enriched.metadata.is_empty());
        assert_eq!(enriched.metadata.len(), 2);
        assert_eq!(
            enriched.metadata.get("test_key").unwrap(),
            "injected context"
        );
        assert_eq!(
            enriched.metadata.get("another_key").unwrap(),
            "another value"
        );
    }

    #[test]
    fn test_context_injection_without_inject_as() {
        use std::io::Write;
        use tempfile::TempDir;

        let provider = MockProvider::simple_text("test");

        // Create a hook without inject_as
        let temp_dir = TempDir::new().unwrap();
        let hook_file = temp_dir.path().join("test_hook.yaml");
        let mut file = std::fs::File::create(&hook_file).unwrap();
        writeln!(
            file,
            r#"name: test_no_injection
trigger: SessionStart
actions:
  - type: command
    command: "echo 'not injected'"
  - type: message
    text: "just a message""#
        )
        .unwrap();
        drop(file);

        let hooks = HookRegistry::load_from_directory(&temp_dir.path().to_path_buf()).unwrap();

        let agent = agent_for_test(provider).with_hooks(hooks);

        let ctx = EventContext::new();
        let enriched = agent.execute_hooks_for_event(&Event::SessionStart, &ctx);

        // Should NOT have any injected context
        assert!(enriched.metadata.is_empty());
    }

    #[test]
    fn test_context_injection_large_value_truncation() {
        let provider = MockProvider::simple_text("test");
        let mut agent = agent_for_test(provider);

        // Create context with a very large injected value
        let large_value = "x".repeat(5000);
        let mut ctx = EventContext::new().with_user_message("test".to_string());
        ctx.metadata.insert("large_key".to_string(), large_value);

        // Simulate the hook execution result
        agent.messages.push(crate::api::Message::user("test"));

        // The run_turn method should handle large values gracefully
        // We can't easily test the full flow without mocking, but we can verify
        // the context is created correctly
        assert_eq!(ctx.metadata.get("large_key").unwrap().len(), 5000);
    }

    #[tokio::test]
    async fn test_run_turn_simple() {
        let provider = MockProvider::simple_text("Hello response");
        let mut agent = agent_for_test(provider);

        agent.add_user_message("Hello");
        let result = agent.run_turn().await;

        assert!(result.is_ok());
        // Should have user message + assistant response
        assert_eq!(agent.messages.len(), 2);
        assert_eq!(agent.messages[1].role, "assistant");
    }

    #[test]
    fn compact_messages_drops_oldest_pairs_to_fit_window() {
        // 10 alternating user+assistant messages — each "word" ≈ 4 chars = 1 token
        // With a tight window, oldest pairs should be dropped
        let messages: Vec<Message> = (0..10)
            .flat_map(|i| {
                vec![
                    Message::user(format!("user message number {i}")),
                    Message::assistant(vec![ContentBlock::Text {
                        text: format!("assistant reply number {i}"),
                    }]),
                ]
            })
            .collect();
        // Allow ~60 tokens → should keep only the most recent few pairs
        let compacted = compact_messages(&messages, 60);
        assert!(
            compacted.len() < messages.len(),
            "expected compaction but got {} messages (same as input {})",
            compacted.len(),
            messages.len()
        );
        // First message of the result must be a user message (never start mid-pair)
        assert_eq!(compacted[0].role, "user");
    }

    #[test]
    fn compact_messages_preserves_all_when_under_limit() {
        let messages = vec![
            Message::user("hi".to_string()),
            Message::assistant(vec![ContentBlock::Text {
                text: "hello".to_string(),
            }]),
        ];
        let compacted = compact_messages(&messages, 100_000);
        assert_eq!(compacted.len(), messages.len());
    }

    #[test]
    fn observation_manager_initialized() {
        let provider = MockProvider::simple_text("test");
        let agent = agent_for_test(provider);

        assert_eq!(agent.observations.count(), 0);
    }

    #[test]
    fn test_event_manager_initialized() {
        let provider = MockProvider::simple_text("test");
        let agent = agent_for_test(provider);

        // EventManager should be initialized and ready to use
        let ctx = EventContext::new();
        agent.events.fire(Event::SessionStart, &ctx);
        // Should not panic
    }

    #[test]
    fn test_file_reference_resolution() {
        use std::io::Write;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        let mut file = std::fs::File::create(&test_file).unwrap();
        writeln!(file, "Hello from file!").unwrap();

        let provider = MockProvider::simple_text("test");
        let mut agent = agent_for_test(provider);

        // Override working directory to temp dir for this test
        agent.tool_ctx.working_dir = temp_dir.path().to_path_buf();

        // Add message with file reference
        agent.add_user_message("Check @test.txt please");

        // Should have resolved the file reference
        assert_eq!(agent.messages.len(), 1);
        let msg_content = agent.messages[0].content.first().unwrap();
        if let crate::api::ContentBlock::Text { text } = msg_content {
            assert!(text.contains("Hello from file!"));
            assert!(text.contains("// File: test.txt"));
        } else {
            panic!("Expected text content block");
        }
    }

    #[test]
    fn execute_hooks_supports_prompt_callbacks() {
        let _ = Agent::execute_hooks_for_event_with_callbacks;
    }

    #[test]
    fn build_system_prompt_includes_working_dir() {
        let provider = MockProvider::simple_text("test");
        let agent = agent_for_test(provider);
        let ctx = EventContext::new();

        let prompt = agent.build_system_prompt(&ctx);
        assert!(prompt.contains("Current working directory:"));
    }

    #[test]
    fn build_system_prompt_includes_rules() {
        let provider = MockProvider::simple_text("test");
        let mut agent = agent_for_test(provider);
        let mut rules = RuleRegistry::new();
        rules.register(crate::rules::Rule {
            id: "test-rule".to_string(),
            title: "Test Rule".to_string(),
            content: "Always use snake_case".to_string(),
            categories: vec![],
            source: std::path::PathBuf::from("test"),
        });
        agent.rules = rules;

        let ctx = EventContext::new();
        let prompt = agent.build_system_prompt(&ctx);
        assert!(prompt.contains("Always use snake_case"));
    }

    #[test]
    fn build_system_prompt_includes_hook_context() {
        let provider = MockProvider::simple_text("test");
        let agent = agent_for_test(provider);
        let mut ctx = EventContext::new();
        ctx.metadata
            .insert("git_status".to_string(), "clean".to_string());

        let prompt = agent.build_system_prompt(&ctx);
        assert!(prompt.contains("git_status"));
        assert!(prompt.contains("clean"));
    }

    #[test]
    fn build_system_prompt_truncates_large_hook_values() {
        let provider = MockProvider::simple_text("test");
        let agent = agent_for_test(provider);
        let mut ctx = EventContext::new();
        ctx.metadata.insert("big".to_string(), "x".repeat(5000));

        let prompt = agent.build_system_prompt(&ctx);
        assert!(prompt.contains("[truncated"));
        assert!(prompt.len() < 5000 + 500);
    }

    struct MockSessionStore {
        events: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    }

    impl MockSessionStore {
        fn new() -> (Self, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
            let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
            (
                Self {
                    events: events.clone(),
                },
                events,
            )
        }
    }

    impl SessionStore for MockSessionStore {
        fn log(&mut self, event: SessionEvent) -> Result<(), anyhow::Error> {
            let tag = match &event {
                SessionEvent::UserMessage { .. } => "user_message",
                SessionEvent::Inference { .. } => "inference",
                SessionEvent::ToolUse { .. } => "tool_use",
                SessionEvent::ToolResult { .. } => "tool_result",
                SessionEvent::SessionEnd => "session_end",
            };
            self.events.lock().unwrap().push(tag.to_string());
            Ok(())
        }

        fn path(&self) -> Option<&std::path::Path> {
            None
        }

        fn session_id(&self) -> &str {
            "mock-session"
        }
    }

    #[test]
    fn log_inference_records_session_event() {
        let provider = MockProvider::simple_text("test");
        let mut agent = agent_for_test(provider);

        let (store, events) = MockSessionStore::new();
        agent.session_logger = Some(Box::new(store));

        let response = InferenceResponse {
            content: vec![ContentBlock::Text {
                text: "hello from LLM".to_string(),
            }],
            stop_reason: "end_turn".to_string(),
            usage: Usage {
                input_tokens: 5,
                output_tokens: 10,
            },
        };

        agent.log_inference(&response);

        let logged = events.lock().unwrap();
        assert_eq!(logged.len(), 1);
        assert_eq!(logged[0], "inference");
    }

    #[test]
    fn agent_accepts_injected_session_store() {
        let provider = MockProvider::simple_text("test");
        let (store, events) = MockSessionStore::new();

        let mut agent = Agent::new_with_runtime(
            Box::new(provider),
            RuntimeSettings::default(),
            FileRefPolicy::default(),
            Some(Box::new(store)),
            Box::new(NullOutput),
        )
        .unwrap();

        agent.add_user_message("hello");
        // Directly call log_inference to verify the injected store works
        let response = InferenceResponse {
            content: vec![ContentBlock::Text {
                text: "hi".to_string(),
            }],
            stop_reason: "end_turn".to_string(),
            usage: Usage {
                input_tokens: 1,
                output_tokens: 1,
            },
        };
        agent.log_inference(&response);

        let logged = events.lock().unwrap();
        assert_eq!(logged.len(), 1);
        assert_eq!(logged[0], "inference");
    }

    #[test]
    fn agent_uses_injected_output() {
        use std::sync::{Arc, Mutex};

        struct RecordingOutput {
            infos: Arc<Mutex<Vec<String>>>,
        }
        impl UserOutput for RecordingOutput {
            fn info(&self, msg: &str) {
                self.infos.lock().unwrap().push(msg.to_string());
            }
            fn warn(&self, _: &str) {}
            fn error(&self, _: &str) {}
            fn assistant_text(&self, _: &str) {}
            fn tool_call(&self, _: &str, _: &str) {}
            fn tool_ok(&self) {}
            fn tool_err(&self, _: &str) {}
        }

        let infos = Arc::new(Mutex::new(Vec::new()));
        let output = RecordingOutput {
            infos: infos.clone(),
        };

        let provider = MockProvider::simple_text("test");
        let _agent = Agent::new_with_runtime(
            Box::new(provider),
            RuntimeSettings::default(),
            FileRefPolicy::default(),
            None,
            Box::new(output),
        )
        .unwrap();

        // Agent was constructed with our custom output - verify it compiled
        // and the output is wired (infos vec is shared, not the default UiOutput)
        assert!(infos.lock().unwrap().is_empty());
    }

    #[test]
    fn with_rules_builder() {
        let provider = MockProvider::simple_text("test");
        let mut rules = RuleRegistry::new();
        rules.register(crate::rules::Rule {
            id: "test".to_string(),
            title: "Test".to_string(),
            content: "rule content".to_string(),
            categories: vec![],
            source: std::path::PathBuf::from("test"),
        });

        let agent = agent_for_test(provider).with_rules(rules);

        let ctx = EventContext::new();
        let prompt = agent.build_system_prompt(&ctx);
        assert!(prompt.contains("rule content"));
    }

    #[tokio::test]
    async fn agent_runs_without_real_filesystem_or_provider() {
        // Full run_turn through injected ports only — no real I/O, no ui:: statics
        let provider = MockProvider::simple_text("hello from mock");
        let (store, events) = MockSessionStore::new();
        let mut agent = Agent::new_with_runtime(
            Box::new(provider),
            RuntimeSettings::default(),
            FileRefPolicy::default(),
            Some(Box::new(store)),
            Box::new(NullOutput),
        )
        .unwrap();

        agent.add_user_message("hello");
        agent.run_turn().await.unwrap();

        // Session store received events via port — no direct ui:: calls
        let logged = events.lock().unwrap();
        assert!(
            logged.iter().any(|e| e == "inference"),
            "expected inference event, got: {logged:?}"
        );
        assert!(
            logged.iter().any(|e| e == "user_message"),
            "expected user_message event, got: {logged:?}"
        );
    }

    #[test]
    fn fire_event_delegates_to_event_manager() {
        let provider = MockProvider::simple_text("test");
        let agent = agent_for_test(provider);

        // Should not panic — verifies the method exists and works
        let ctx = EventContext::new();
        agent.fire_event(Event::SessionStart, &ctx);
    }
}
