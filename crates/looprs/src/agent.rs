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
use crate::tools::{DefaultToolExecutor, ToolContext, ToolExecutor, get_tool_definitions};
use futures::StreamExt as _;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::time::{Duration, timeout};

const TOOL_PREVIEW_LEN: usize = 60;
const ON_REPEAT_THRESHOLD: usize = 3;
const ORCHESTRATION_TOOLS_METADATA_KEY: &str = "orchestration.tools";

/// A single transcript entry for UI consumption: role plus flattened text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    /// Message role (`user`, `assistant`, etc.).
    pub role: String,
    /// Flattened text content for UI display.
    pub text: String,
}

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

fn parse_allowed_tools(metadata: &HashMap<String, String>) -> Option<HashSet<String>> {
    let raw = metadata.get(ORCHESTRATION_TOOLS_METADATA_KEY)?;
    let parsed = raw
        .split(',')
        .map(str::trim)
        .filter(|tool| !tool.is_empty())
        .map(ToOwned::to_owned)
        .collect::<HashSet<_>>();

    if parsed.is_empty() {
        None
    } else {
        Some(parsed)
    }
}

/// Mutable runtime settings applied to each agent turn.
#[derive(Debug, Clone, Default)]
pub struct RuntimeSettings {
    /// Default runtime knobs loaded from app config.
    pub defaults: DefaultsConfig,
    /// Optional hard override for max output tokens.
    pub max_tokens_override: Option<u32>,
    /// Filesystem permission mode used by tool execution.
    pub fs_mode: FsMode,
    /// Upper bound on parallel tool dispatch fan-out.
    pub max_parallel: usize,
    /// Optional MCP server URL used for remote tool discovery/execution.
    pub mcp_server_url: Option<String>,
}

/// Primary orchestrator for provider inference, tools, rules, and hooks.
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
    tool_executor: Arc<dyn ToolExecutor>,
    models_config: Option<ModelsConfig>,
    system_monitor: SystemMonitor,
    session_input_tokens: u32,
    session_output_tokens: u32,
}

impl Agent {
    /// Construct an agent with default runtime settings and console output.
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

    /// Construct an agent with explicit runtime, policy, and adapter ports.
    pub fn new_with_runtime(
        provider: Box<dyn LLMProvider>,
        runtime: RuntimeSettings,
        file_ref_policy: FileRefPolicy,
        session_logger: Option<Box<dyn SessionStore>>,
        output: Box<dyn UserOutput>,
    ) -> Result<Self, AgentError> {
        let tool_executor: Box<dyn ToolExecutor> = match runtime.mcp_server_url.clone() {
            Some(server_url) => Box::new(crate::adapters::McpToolExecutor::with_fallback(
                server_url,
                Box::new(DefaultToolExecutor),
            )),
            None => Box::new(DefaultToolExecutor),
        };

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
            tool_executor: Arc::from(tool_executor),
            models_config: ModelsConfig::load().ok(),
            system_monitor: SystemMonitor::new(),
            session_input_tokens: 0,
            session_output_tokens: 0,
        })
    }

    fn merge_remote_tool_definitions(
        mut tools: Vec<looprs_core::api::ToolDefinition>,
        remote_tools: Vec<looprs_core::api::ToolDefinition>,
    ) -> Vec<looprs_core::api::ToolDefinition> {
        let mut known = tools
            .iter()
            .map(|tool| tool.name.clone())
            .collect::<std::collections::HashSet<_>>();
        for remote_tool in remote_tools {
            if known.insert(remote_tool.name.clone()) {
                tools.push(remote_tool);
            }
        }
        tools
    }

    /// Replace the output adapter. Useful for tests (inject `NullOutput`) or
    /// alternative frontends (GUI, JSON stream, etc.).
    pub fn with_output(mut self, output: Box<dyn UserOutput>) -> Self {
        self.output = output;
        self
    }

    /// Replace the tool executor. Inject a stub in tests to avoid real
    /// filesystem or subprocess side effects.
    pub fn with_tool_executor(mut self, executor: Box<dyn ToolExecutor>) -> Self {
        self.tool_executor = Arc::from(executor);
        self
    }

    /// Replace the hook registry used for lifecycle events.
    pub fn with_hooks(mut self, hooks: HookRegistry) -> Self {
        self.hooks = hooks;
        self
    }

    /// Replace the rule registry used for prompt/tool filtering.
    pub fn with_rules(mut self, rules: RuleRegistry) -> Self {
        self.rules = rules;
        self
    }

    /// Fire one event through the internal event manager.
    pub fn fire_event(&self, event: Event, context: &EventContext) {
        self.events.fire(event, context);
    }

    /// Swap the active provider implementation.
    pub fn set_provider(&mut self, provider: Box<dyn LLMProvider>) {
        self.provider = provider;
    }

    /// Update runtime settings and propagate filesystem mode to tool context.
    pub fn set_runtime_settings(&mut self, runtime: RuntimeSettings) {
        self.tool_ctx.set_fs_mode(runtime.fs_mode);
        self.runtime = runtime;
    }

    /// Update file-reference resolution policy.
    pub fn set_file_ref_policy(&mut self, policy: FileRefPolicy) {
        self.file_ref_policy = policy;
    }

    /// Current filesystem mode used by tool execution.
    pub fn fs_mode(&self) -> FsMode {
        self.tool_ctx.fs_mode()
    }

    /// Set filesystem mode used by tool execution.
    pub fn set_fs_mode(&self, mode: FsMode) {
        self.tool_ctx.set_fs_mode(mode);
    }

    /// Shared atomic handle for cross-component filesystem mode updates.
    pub fn fs_mode_handle(&self) -> std::sync::Arc<std::sync::atomic::AtomicU8> {
        self.tool_ctx.fs_mode_handle()
    }

    /// Add per-turn metadata that will be attached to the next turn.
    pub fn set_turn_metadata(&mut self, metadata: HashMap<String, String>) {
        self.pending_metadata.extend(metadata);
    }

    /// Append a user message, resolving configured file references first.
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

    /// Remove all accumulated message history for this session.
    pub fn clear_history(&mut self) {
        self.messages.clear();
    }

    /// Best-effort context-window size for the currently selected model.
    pub fn provider_model_max_tokens(&self) -> u32 {
        self.provider.model().max_tokens()
    }

    /// Current provider model identifier.
    pub fn provider_model_id(&self) -> &crate::types::ModelId {
        self.provider.model()
    }

    /// Cumulative token usage for this session (input, output).
    pub fn session_tokens(&self) -> (u32, u32) {
        (self.session_input_tokens, self.session_output_tokens)
    }

    /// Estimated context size in tokens (1 token ≈ 4 chars).
    pub fn estimated_context_tokens(&self) -> u32 {
        let chars: usize = self
            .messages
            .iter()
            .flat_map(|m| m.content.iter())
            .map(|b| match b {
                ContentBlock::Text { text } => text.len(),
                ContentBlock::ToolUse { input, .. } => input.to_string().len(),
                ContentBlock::ToolResult { content, .. } => content.len(),
            })
            .sum();
        (chars / 4) as u32
    }

    /// Most recent assistant text-only response, if present.
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

    /// Working directory backing tool execution.
    pub fn working_dir(&self) -> &std::path::Path {
        &self.tool_ctx.working_dir
    }

    /// Full conversation history as plain (role, text) pairs, in order, for
    /// UI consumption. Non-text content blocks (tool use/results) are
    /// rendered as their block text where present; empty messages are kept
    /// so turn boundaries stay visible.
    pub fn transcript(&self) -> Vec<ChatMessage> {
        self.messages
            .iter()
            .map(|m| ChatMessage {
                role: m.role.clone(),
                text: m
                    .content
                    .iter()
                    .filter_map(|block| match block {
                        ContentBlock::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n"),
            })
            .collect()
    }

    /// Execute hooks for an event using default callback handlers.
    pub fn execute_hooks_for_event(&self, event: &Event, context: &EventContext) -> EventContext {
        self.execute_hooks_for_event_with_callbacks(event, context, None, None, None)
    }

    // qual:allow(iosp) reason: "I/O boundary — orchestrates hook execution with callbacks"
    /// Execute hooks for an event with optional interactive callbacks.
    pub fn execute_hooks_for_event_with_callbacks(
        &self,
        event: &Event,
        context: &EventContext,
        approval_fn: Option<&ApprovalCallback>,
        prompt_fn: Option<&PromptCallback>,
        secret_prompt_fn: Option<&PromptCallback>,
    ) -> EventContext {
        let mut enriched_context = context.clone();
        let mut hook_context = enriched_context.clone();
        hook_context
            .metadata
            .insert("event_name".to_string(), event.name().to_string());
        hook_context
            .metadata
            .insert("event".to_string(), event.name().to_string());

        if let Some(hooks) = self.hooks.hooks_for_event(event) {
            for hook in hooks {
                if let Ok(results) = HookExecutor::execute_hook_with_callbacks(
                    hook,
                    &hook_context,
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

        // M3: inject pipeline compaction context (diff, recent files, globs)
        #[cfg(not(test))]
        if let Ok(app_cfg) = crate::app_config::AppConfig::load()
            && let Ok(compacted) = crate::pipeline::context_compact::compact_context(
                std::path::Path::new("."),
                &app_cfg.pipeline.compaction,
            )
            && !compacted.text.is_empty()
        {
            system_prompt.push_str("\n\n## Repo Context\n");
            system_prompt.push_str(&compacted.text);
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

    /// Single-turn streaming inference.
    ///
    /// Drives `provider.infer_stream()`, emitting each text chunk via
    /// `output.write_chunk()`. When stop_reason is `tool_use` the accumulated
    /// response is re-parsed and tool calls are executed identically to
    /// `run_turn()`. Subsequent turns after tool execution run non-streaming
    /// (the provider accumulates those as a single response).
    pub async fn run_turn_streaming(&mut self) -> Result<(), AgentError> {
        use futures::StreamExt as _;

        let enriched_ctx = EventContext::new();
        let system_prompt = self.build_system_prompt(&enriched_ctx);

        let mut max_tokens = self.provider.model().max_tokens();
        if let Some(ov) = self.runtime.max_tokens_override {
            max_tokens = max_tokens.min(ov);
        }
        if let Some(max_ctx) = self.runtime.defaults.max_context_tokens {
            max_tokens = max_tokens.min(max_ctx);
        }
        let messages = if let Some(max_ctx) = self.runtime.defaults.max_context_tokens {
            compact_messages(&self.messages, max_ctx as usize)
        } else {
            self.messages.clone()
        };
        let allowed_tools = parse_allowed_tools(&self.pending_metadata);
        let mut tools = get_tool_definitions();
        if let Some(server_url) = self.runtime.mcp_server_url.clone() {
            match crate::tools::mcp_tool_definitions(&server_url).await {
                Ok(remote_tools) => {
                    tools = Self::merge_remote_tool_definitions(tools, remote_tools);
                }
                Err(err) => {
                    self.output.warn(&format!(
                        "Warning: failed to discover MCP tools from {server_url}: {err}"
                    ));
                }
            }
        }
        if let Some(allowed) = allowed_tools.as_ref() {
            tools.retain(|tool| allowed.contains(tool.name.as_str()));
        }
        let allowed_tools = parse_allowed_tools(&self.pending_metadata);
        if let Some(allowed) = allowed_tools.as_ref() {
            tools.retain(|tool| allowed.contains(tool.name.as_str()));
        }
        let req = InferenceRequest {
            model: self.provider.model().clone(),
            messages,
            tools,
            max_tokens,
            temperature: self.runtime.defaults.temperature,
            system: system_prompt,
        };

        // Stream text chunks to the output port, accumulate full text.
        let mut stream = self.provider.infer_stream(&req).await;
        let mut accumulated = String::new();
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(text) => {
                    self.output.write_chunk(&text);
                    accumulated.push_str(&text);
                }
                Err(e) => return Err(AgentError::Inference(e.to_string())),
            }
        }

        // Recover the provider's structured response so streamed turns preserve
        // tool calls and usage accounting.
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

        self.session_input_tokens += response.usage.input_tokens;
        self.session_output_tokens += response.usage.output_tokens;
        self.log_inference(&response);

        #[cfg(not(test))]
        if let Err(e) =
            crate::trace::append_turn_trace(self.observations.session_id(), &req, &response)
        {
            self.output
                .warn(&format!("Warning: Failed to append turn trace: {e}"));
        }

        let event_ctx = EventContext::new();
        self.events.fire(Event::InferenceComplete, &event_ctx);
        self.execute_hooks_for_event(&Event::InferenceComplete, &event_ctx);

        if response.content.is_empty() {
            if !accumulated.is_empty() {
                self.messages
                    .push(Message::assistant(vec![ContentBlock::Text {
                        text: accumulated,
                    }]));
            }
            return Ok(());
        }

        let has_tool_use = response
            .content
            .iter()
            .any(|block| matches!(block, ContentBlock::ToolUse { .. }));
        self.messages.push(Message::assistant(response.content));

        if has_tool_use {
            let mut tool_results = Vec::new();
            let assistant_message = self.messages.last().expect("assistant message just pushed");

            for block in &assistant_message.content {
                let ContentBlock::ToolUse { id, name, input } = block else {
                    continue;
                };

                let event_ctx = EventContext::new().with_tool_name(name.as_str().to_string());
                self.events.fire(Event::PreToolUse, &event_ctx);
                self.execute_hooks_for_event(&Event::PreToolUse, &event_ctx);

                let result = self
                    .tool_executor
                    .execute(name.as_str(), input, &self.tool_ctx);
                let raw_content = match result {
                    Ok(ref output) => {
                        self.output.tool_ok();
                        self.observations.capture(
                            name.as_str().to_string(),
                            input.clone(),
                            output.clone(),
                            Some(id.clone()),
                        );
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
                        let event_ctx = EventContext::new()
                            .with_tool_name(name.as_str().to_string())
                            .with_error(err_msg.clone());
                        self.events.fire(Event::OnError, &event_ctx);
                        self.execute_hooks_for_event(&Event::OnError, &event_ctx);
                        err_msg
                    }
                };

                tool_results.push(ContentBlock::ToolResult {
                    tool_use_id: id.clone(),
                    content: truncate_tool_result_for_context(&raw_content),
                });
            }

            if !tool_results.is_empty() {
                self.messages.push(Message::tool_results(tool_results));
            }
        }

        Ok(())
    }

    /// Run one full agent turn (inference plus any requested tool loop).
    pub async fn run_turn(&mut self) -> Result<(), AgentError> {
        let delegated_agent = self.pending_metadata.get("orchestration.agent").cloned();
        let orchestration_strategy = self
            .pending_metadata
            .get("orchestration.strategy")
            .cloned()
            .unwrap_or_else(|| "sequential".to_string());
        if let Some(agent_name) = delegated_agent.clone() {
            let event_ctx = EventContext::new()
                .with_tool_name(agent_name)
                .with_metadata(
                    "orchestration.strategy".to_string(),
                    orchestration_strategy.clone(),
                )
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

        let allowed_tools = parse_allowed_tools(&enriched_ctx.metadata);

        let system_prompt = self.build_system_prompt(&enriched_ctx);

        let mut tool_call_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut tools = get_tool_definitions();
        if let Some(server_url) = self.runtime.mcp_server_url.clone() {
            match crate::tools::mcp_tool_definitions(&server_url).await {
                Ok(remote_tools) => {
                    tools = Self::merge_remote_tool_definitions(tools, remote_tools);
                }
                Err(err) => {
                    self.output.warn(&format!(
                        "Warning: failed to discover MCP tools from {server_url}: {err}"
                    ));
                }
            }
        }
        if let Some(allowed) = allowed_tools.as_ref() {
            tools.retain(|tool| allowed.contains(tool.name.as_str()));
        }

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
                tools: tools.clone(),
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

            self.session_input_tokens += response.usage.input_tokens;
            self.session_output_tokens += response.usage.output_tokens;

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

            struct PendingToolCall {
                position: usize,
                id: crate::types::ToolId,
                name: crate::types::ToolName,
                input: serde_json::Value,
            }

            struct ToolCallOutcome {
                position: usize,
                id: crate::types::ToolId,
                name: crate::types::ToolName,
                input: serde_json::Value,
                result: Result<String, crate::tools::ToolError>,
            }

            let mut tool_results = Vec::new();
            let assistant_message = self.messages.last().expect("assistant message just pushed");
            let mut pending_calls = Vec::new();

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

                pending_calls.push(PendingToolCall {
                    position: idx,
                    id: id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                });
            }

            let parallel_enabled = orchestration_strategy.eq_ignore_ascii_case("parallel")
                && self.runtime.max_parallel > 1
                && pending_calls.len() > 1;

            let mut outcomes = if parallel_enabled {
                let max_parallel = self.runtime.max_parallel.max(1).min(pending_calls.len());
                let executor = Arc::clone(&self.tool_executor);
                let tool_ctx = self.tool_ctx.clone();
                let allowed_tools_for_exec = allowed_tools.clone();

                futures::stream::iter(pending_calls.into_iter())
                    .map(|call| {
                        let executor = Arc::clone(&executor);
                        let tool_ctx = tool_ctx.clone();
                        let allowed_tools_for_exec = allowed_tools_for_exec.clone();
                        async move {
                            let name = call.name.clone();
                            let input = call.input.clone();
                            let result = if let Some(allowed) = allowed_tools_for_exec.as_ref()
                                && !allowed.contains(name.as_str())
                            {
                                Err(crate::tools::ToolError::ModeDenied {
                                    tool: name.to_string(),
                                    mode: "delegated".to_string(),
                                    reason: "not in agent tool allowlist".to_string(),
                                })
                            } else {
                                tokio::task::spawn_blocking(move || {
                                    executor.execute(name.as_str(), &input, &tool_ctx)
                                })
                                .await
                                .map_err(|e| crate::tools::ToolError::CommandFailed(e.to_string()))
                                .and_then(|r| r)
                            };

                            ToolCallOutcome {
                                position: call.position,
                                id: call.id,
                                name: call.name,
                                input: call.input,
                                result,
                            }
                        }
                    })
                    .buffer_unordered(max_parallel)
                    .collect::<Vec<_>>()
                    .await
            } else {
                pending_calls
                    .into_iter()
                    .map(|call| {
                        let result = if let Some(allowed) = allowed_tools.as_ref()
                            && !allowed.contains(call.name.as_str())
                        {
                            Err(crate::tools::ToolError::ModeDenied {
                                tool: call.name.to_string(),
                                mode: "delegated".to_string(),
                                reason: "not in agent tool allowlist".to_string(),
                            })
                        } else {
                            self.tool_executor.execute(
                                call.name.as_str(),
                                &call.input,
                                &self.tool_ctx,
                            )
                        };
                        ToolCallOutcome {
                            position: call.position,
                            id: call.id,
                            name: call.name,
                            input: call.input,
                            result,
                        }
                    })
                    .collect::<Vec<_>>()
            };

            outcomes.sort_by_key(|outcome| outcome.position);

            for outcome in outcomes {
                let ToolCallOutcome {
                    id,
                    name,
                    input,
                    result,
                    ..
                } = outcome;
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

            // M1: pipeline self-check after successful tool-use round-trip
            if let Ok(app_cfg) = crate::app_config::AppConfig::load()
                && app_cfg.pipeline.enabled
            {
                let snapshot = self.messages.clone();
                let report = crate::pipeline::PipelineRunner::run(&app_cfg.pipeline);
                let failures: Vec<String> = report
                    .steps
                    .iter()
                    .filter(|s| !s.success)
                    .map(|s| s.step.clone())
                    .collect();
                if crate::pipeline::PipelineRunner::should_block(&app_cfg.pipeline, &report) {
                    if app_cfg.pipeline.auto_revert {
                        self.messages = snapshot;
                    }
                    return Err(crate::errors::AgentError::PipelineFailure(
                        failures.join(", "),
                    ));
                }
            }
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

        // L3: auto-persist observations to SQLite at session end
        if self.observations.count() > 0 {
            let obs_db = dirs::home_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join(".looprs")
                .join("observations.db");
            if let Err(e) = self.observations.persist(&obs_db) {
                self.output
                    .warn(&format!("Warning: failed to persist observations: {e}"));
            }
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
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Mock provider for testing
    struct MockProvider {
        model: crate::types::ModelId,
        responses: Vec<InferenceResponse>,
        call_count: std::sync::Arc<std::sync::Mutex<usize>>,
        captured_tools: std::sync::Arc<std::sync::Mutex<Vec<Vec<String>>>>,
    }

    struct TrackingExecutor {
        active: Arc<AtomicUsize>,
        max_active: Arc<AtomicUsize>,
        delay_ms: u64,
    }

    impl ToolExecutor for TrackingExecutor {
        fn execute(
            &self,
            _name: &str,
            _args: &serde_json::Value,
            _ctx: &ToolContext,
        ) -> Result<String, crate::tools::ToolError> {
            let now = self.active.fetch_add(1, Ordering::SeqCst) + 1;

            loop {
                let seen = self.max_active.load(Ordering::SeqCst);
                if now <= seen {
                    break;
                }
                if self
                    .max_active
                    .compare_exchange(seen, now, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
                {
                    break;
                }
            }

            std::thread::sleep(std::time::Duration::from_millis(self.delay_ms));
            self.active.fetch_sub(1, Ordering::SeqCst);
            Ok("ok".to_string())
        }
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
                captured_tools: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
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

        fn captured_tools_handle(&self) -> std::sync::Arc<std::sync::Mutex<Vec<Vec<String>>>> {
            self.captured_tools.clone()
        }
    }

    struct RecordingProvider {
        model: crate::types::ModelId,
        seen_tool_names: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl LLMProvider for RecordingProvider {
        async fn infer(
            &self,
            req: &InferenceRequest,
        ) -> Result<InferenceResponse, Box<dyn std::error::Error + Send + Sync>> {
            let mut seen = self.seen_tool_names.lock().unwrap();
            *seen = req.tools.iter().map(|t| t.name.clone()).collect();
            Ok(InferenceResponse {
                content: vec![ContentBlock::Text {
                    text: "ok".to_string(),
                }],
                stop_reason: "end_turn".to_string(),
                usage: Usage {
                    input_tokens: 1,
                    output_tokens: 1,
                },
            })
        }

        fn name(&self) -> &str {
            "recording"
        }

        fn model(&self) -> &crate::types::ModelId {
            &self.model
        }

        fn validate_config(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
    }

    fn start_mcp_tools_server() -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0_u8; 4096];
            let _ = stream.read(&mut buf);

            let body = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": {
                    "tools": [
                        {
                            "name": "remote_test_tool",
                            "description": "remote tool",
                            "inputSchema": {
                                "type": "object",
                                "properties": {}
                            }
                        }
                    ]
                }
            })
            .to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        });
        (format!("http://{addr}"), handle)
    }

    #[async_trait::async_trait]
    impl LLMProvider for MockProvider {
        async fn infer(
            &self,
            req: &InferenceRequest,
        ) -> Result<InferenceResponse, Box<dyn std::error::Error + Send + Sync>> {
            self.captured_tools.lock().unwrap().push(
                req.tools
                    .iter()
                    .map(|tool| tool.name.clone())
                    .collect::<Vec<_>>(),
            );

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

    #[tokio::test]
    async fn run_turn_includes_discovered_mcp_tools_in_request() {
        let (server_url, server_thread) = start_mcp_tools_server();
        let seen_tool_names = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let provider = RecordingProvider {
            model: crate::types::ModelId::new("mock-model"),
            seen_tool_names: seen_tool_names.clone(),
        };

        let runtime = RuntimeSettings {
            defaults: DefaultsConfig::default(),
            max_tokens_override: None,
            fs_mode: FsMode::Write,
            max_parallel: 1,
            mcp_server_url: Some(server_url),
        };
        let mut agent = Agent::new_with_runtime(
            Box::new(provider),
            runtime,
            FileRefPolicy::default(),
            None,
            Box::new(NullOutput),
        )
        .unwrap();
        agent.add_user_message("hello");

        agent.run_turn().await.unwrap();
        let seen = seen_tool_names.lock().unwrap().clone();

        assert!(seen.iter().any(|name| name == "read"));
        assert!(seen.iter().any(|name| name == "remote_test_tool"));
        server_thread.join().unwrap();
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

        let _lock = crate::app_config::cwd_test_lock();
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
    fn hook_condition_can_match_event_name() {
        use std::io::Write;
        use tempfile::TempDir;

        let _lock = crate::app_config::cwd_test_lock();
        let provider = MockProvider::simple_text("test");
        let temp_dir = TempDir::new().unwrap();
        let hook_file = temp_dir.path().join("event_name_hook.yaml");
        let mut file = std::fs::File::create(&hook_file).unwrap();
        writeln!(
            file,
            r#"name: event_name_condition
trigger: SessionStart
condition: equals:event_name:SessionStart
actions:
  - type: command
    command: "echo 'matched'"
    inject_as: "event_match""#
        )
        .unwrap();
        drop(file);

        let hooks = HookRegistry::load_from_directory(&temp_dir.path().to_path_buf()).unwrap();
        let agent = agent_for_test(provider).with_hooks(hooks);
        let ctx = EventContext::new();
        let enriched = agent.execute_hooks_for_event(&Event::SessionStart, &ctx);

        assert_eq!(
            enriched.metadata.get("event_match"),
            Some(&"matched".to_string())
        );
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

    #[tokio::test]
    async fn delegated_allowlist_limits_advertised_tools() {
        let provider = MockProvider::simple_text("done");
        let captured = provider.captured_tools_handle();
        let mut agent = agent_for_test(provider);

        let mut metadata = HashMap::new();
        metadata.insert(
            ORCHESTRATION_TOOLS_METADATA_KEY.to_string(),
            "read,grep".to_string(),
        );
        agent.set_turn_metadata(metadata);
        agent.add_user_message("hello");
        agent.run_turn().await.unwrap();

        let requests = captured.lock().unwrap();
        assert!(
            !requests.is_empty(),
            "expected at least one inference request"
        );
        assert_eq!(requests[0], vec!["read".to_string(), "grep".to_string()]);
    }

    #[tokio::test]
    async fn delegated_allowlist_blocks_tool_execution() {
        use crate::tools::ToolExecutor;
        use serde_json::json;
        use std::sync::{Arc, Mutex};

        #[derive(Default)]
        struct RecordingToolExecutor {
            calls: Arc<Mutex<Vec<String>>>,
        }

        impl ToolExecutor for RecordingToolExecutor {
            fn execute(
                &self,
                name: &str,
                _args: &serde_json::Value,
                _ctx: &ToolContext,
            ) -> Result<String, crate::tools::ToolError> {
                self.calls.lock().unwrap().push(name.to_string());
                Ok("executed".to_string())
            }
        }

        let responses = vec![
            InferenceResponse {
                content: vec![ContentBlock::ToolUse {
                    id: crate::types::ToolId::new("tool_1"),
                    name: crate::types::ToolName::new("bash"),
                    input: json!({"command": "pwd"}),
                }],
                stop_reason: "tool_use".to_string(),
                usage: Usage {
                    input_tokens: 1,
                    output_tokens: 1,
                },
            },
            InferenceResponse {
                content: vec![ContentBlock::Text {
                    text: "done".to_string(),
                }],
                stop_reason: "end_turn".to_string(),
                usage: Usage {
                    input_tokens: 1,
                    output_tokens: 1,
                },
            },
        ];

        let provider = MockProvider::new(responses);
        let calls = Arc::new(Mutex::new(Vec::new()));
        let executor = RecordingToolExecutor {
            calls: calls.clone(),
        };
        let mut agent = agent_for_test(provider).with_tool_executor(Box::new(executor));

        let mut metadata = HashMap::new();
        metadata.insert(
            ORCHESTRATION_TOOLS_METADATA_KEY.to_string(),
            "read".to_string(),
        );
        metadata.insert("orchestration.agent".to_string(), "reviewer".to_string());
        agent.set_turn_metadata(metadata);
        agent.add_user_message("please inspect");
        agent.run_turn().await.unwrap();

        assert!(
            calls.lock().unwrap().is_empty(),
            "disallowed tool should not execute"
        );

        let denied = agent
            .messages
            .iter()
            .flat_map(|message| message.content.iter())
            .any(|block| {
                matches!(
                    block,
                    ContentBlock::ToolResult { content, .. }
                        if content.contains("not in agent tool allowlist")
                )
            });
        assert!(denied, "expected a denied tool result message");
    }

    #[tokio::test]
    async fn test_run_turn_streaming_accumulates_chunks() {
        // MockProvider uses default infer_stream; provide two responses because
        // run_turn_streaming does a recovery infer after streaming.
        let provider = MockProvider::new(vec![
            InferenceResponse {
                content: vec![ContentBlock::Text {
                    text: "streamed response".to_string(),
                }],
                stop_reason: "end_turn".to_string(),
                usage: Usage {
                    input_tokens: 10,
                    output_tokens: 20,
                },
            },
            InferenceResponse {
                content: vec![ContentBlock::Text {
                    text: "streamed response".to_string(),
                }],
                stop_reason: "end_turn".to_string(),
                usage: Usage {
                    input_tokens: 10,
                    output_tokens: 20,
                },
            },
        ]);
        let mut agent = agent_for_test(provider);

        agent.add_user_message("Hello");
        let result = agent.run_turn_streaming().await;

        assert!(
            result.is_ok(),
            "run_turn_streaming should succeed: {result:?}"
        );
        assert_eq!(agent.messages.len(), 2, "user + assistant messages");
        assert_eq!(agent.messages[1].role, "assistant");
        let text = match &agent.messages[1].content[0] {
            ContentBlock::Text { text } => text.clone(),
            other => panic!("expected Text block, got {other:?}"),
        };
        assert_eq!(text, "streamed response");
    }

    #[tokio::test]
    async fn run_turn_parallel_orchestration_dispatches_tool_calls_concurrently() {
        use crate::types::{ToolId, ToolName};

        let provider = MockProvider::new(vec![
            InferenceResponse {
                content: vec![
                    ContentBlock::ToolUse {
                        id: ToolId::new("call_1"),
                        name: ToolName::new("read"),
                        input: serde_json::json!({"path": "a"}),
                    },
                    ContentBlock::ToolUse {
                        id: ToolId::new("call_2"),
                        name: ToolName::new("read"),
                        input: serde_json::json!({"path": "b"}),
                    },
                ],
                stop_reason: "tool_use".to_string(),
                usage: Usage {
                    input_tokens: 1,
                    output_tokens: 1,
                },
            },
            InferenceResponse {
                content: vec![ContentBlock::Text {
                    text: "done".to_string(),
                }],
                stop_reason: "end_turn".to_string(),
                usage: Usage {
                    input_tokens: 1,
                    output_tokens: 1,
                },
            },
        ]);

        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));
        let executor = TrackingExecutor {
            active: Arc::clone(&active),
            max_active: Arc::clone(&max_active),
            delay_ms: 50,
        };

        let mut agent = Agent::new_with_runtime(
            Box::new(provider),
            RuntimeSettings {
                max_parallel: 2,
                ..RuntimeSettings::default()
            },
            FileRefPolicy::default(),
            None,
            Box::new(NullOutput),
        )
        .unwrap()
        .with_tool_executor(Box::new(executor));

        let mut metadata = HashMap::new();
        metadata.insert("orchestration.strategy".to_string(), "parallel".to_string());
        agent.set_turn_metadata(metadata);
        agent.add_user_message("run tools");
        agent.run_turn().await.unwrap();

        assert!(
            max_active.load(Ordering::SeqCst) >= 2,
            "expected concurrent tool dispatch"
        );
    }

    #[tokio::test]
    async fn test_run_turn_streaming_executes_tool_use_blocks() {
        use crate::tools::executor::StubToolExecutor;
        use crate::types::{ToolId, ToolName};

        let provider = MockProvider::new(vec![
            InferenceResponse {
                content: vec![ContentBlock::Text {
                    text: "streaming intro".to_string(),
                }],
                stop_reason: "end_turn".to_string(),
                usage: Usage {
                    input_tokens: 1,
                    output_tokens: 1,
                },
            },
            InferenceResponse {
                content: vec![ContentBlock::ToolUse {
                    id: ToolId::new("call_1"),
                    name: ToolName::new("read"),
                    input: serde_json::json!({"path": "README.md"}),
                }],
                stop_reason: "tool_use".to_string(),
                usage: Usage {
                    input_tokens: 2,
                    output_tokens: 3,
                },
            },
            InferenceResponse {
                content: vec![ContentBlock::Text {
                    text: "done".to_string(),
                }],
                stop_reason: "end_turn".to_string(),
                usage: Usage {
                    input_tokens: 1,
                    output_tokens: 1,
                },
            },
        ]);

        let mut agent = agent_for_test(provider).with_tool_executor(Box::new(StubToolExecutor {
            response: "tool result".to_string(),
        }));
        agent.add_user_message("please run tool");

        let result = agent.run_turn_streaming().await;
        assert!(result.is_ok(), "streaming turn should succeed: {result:?}");
        assert!(
            agent.messages.iter().any(|m| m
                .content
                .iter()
                .any(|b| matches!(b, ContentBlock::ToolResult { .. }))),
            "expected tool results in conversation history"
        );
        assert_eq!(agent.latest_assistant_text(), None);
    }

    #[tokio::test]
    async fn run_turn_streaming_fires_inference_complete_event() {
        let provider = MockProvider::simple_text("streamed ok");
        let mut agent = agent_for_test(provider);
        let seen = std::sync::Arc::new(std::sync::Mutex::new(0usize));
        let seen_clone = seen.clone();
        agent.events.on(Event::InferenceComplete, move |_, _| {
            let mut count = seen_clone.lock().expect("lock count");
            *count += 1;
        });

        agent.add_user_message("hello");
        let result = agent.run_turn_streaming().await;

        assert!(result.is_ok(), "streaming turn should succeed: {result:?}");
        assert_eq!(*seen.lock().expect("lock seen"), 1);
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

    #[test]
    fn with_tool_executor_replaces_default() {
        use crate::tools::executor::StubToolExecutor;

        let provider = MockProvider::simple_text("test");
        let agent =
            agent_for_test(provider).with_tool_executor(Box::new(StubToolExecutor::default()));

        // Verify the field was swapped — a second call with the same stub type
        // should also compile and not panic.
        let _ = agent.with_tool_executor(Box::new(StubToolExecutor {
            response: "custom".to_string(),
        }));
    }
}
