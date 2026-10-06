//! looprs runtime crate.
//!
//! This crate provides the agent runtime, provider integrations, tool
//! execution, configuration, and extension systems used by both CLI and
//! library consumers.

/// Runtime adapters for messaging, output, retries, and persistence.
pub mod adapters;
mod agent;
/// Agent role definitions and registry loading.
pub mod agents;
mod api;
/// Application-level configuration schema and defaults.
pub mod app_config;
/// Interactive approval and prompt callback utilities.
pub mod approval;
/// Generated BAML client bindings and wrappers.
pub mod baml_client;
/// Slash-command schema and command registry.
pub mod commands;
mod config;
mod config_file;
/// Session context builders and context sources.
pub mod context;
/// doob task integration helpers.
pub mod doob;
/// Error types returned by the runtime and providers.
pub mod errors;
/// Event manager and event payload types.
pub mod events;
/// File-reference parsing and resolution helpers.
pub mod file_refs;
/// Filesystem access mode policy for tool execution.
pub mod fs_mode;
/// Git workspace metadata helpers.
pub mod git_info;
/// Hook definitions, parsing, and execution runtime.
pub mod hooks;
/// Model badge rendering helpers for UI surfaces.
pub mod model_badge;
/// Remote/local model catalog aggregation and rendering.
pub mod model_catalog;
/// `models.toml` loading and tier configuration.
pub mod models_config;
/// Structured observability event writer and paths.
pub mod observability;
/// Observation value type for tool execution traces.
pub mod observation;
/// Observation storage and retrieval manager.
pub mod observation_manager;
/// Optional deterministic pipeline execution framework.
pub mod pipeline;
/// Plugin manifests, loading, and execution helpers.
pub mod plugins;
/// Runtime-local ports and compatibility bridges.
pub mod ports;
/// LLM provider implementations and provider selection.
pub mod providers;
/// Rule definitions and runtime rule registry.
pub mod rules;
/// Log and output sanitization utilities.
pub mod sanitize;
/// Scoring utilities for quality and health signals.
pub mod scorer;
/// Repository scaffolding and seed-template helpers.
pub mod seed;
/// Session log persistence helpers.
pub mod session_log;
/// Shell execution utilities and command wrapping.
pub mod shell;
/// Skill manifest parsing and skill registry.
pub mod skills;
/// Runtime state and state-transition helpers.
pub mod state;
/// System resource monitoring helpers.
pub mod system_monitor;
mod tools;
/// Trace data types for runtime telemetry.
pub mod trace;
/// Shared runtime identifiers and public types.
pub mod types;
/// Terminal UI components and rendering helpers.
pub mod ui;

/// Default message-broker implementation.
pub use crate::adapters::{
    ChannelBroker, NullOutput, PluginsAdapter, RetryProvider, SqliteSessionStore,
};
/// Primary agent runtime type and chat/runtime models.
// TODO(automation-2): Keep external automation behind a versioned process protocol so Crux
// does not couple to looprs release cadence, Tokio runtime choices, or feature graph.
pub use crate::agent::{Agent, ChatMessage, RuntimeSettings};
/// Agent definition schema and agent registry.
pub use crate::agents::{AgentDefinition, AgentRegistry};
/// Default interactive callbacks for approvals and prompts.
pub use crate::approval::{console_approval_prompt, console_prompt, console_secret_prompt};
/// Command schema and command registry.
pub use crate::commands::{Command, CommandAction, CommandRegistry};
/// Provider config schema loaded from config files.
pub use crate::config_file::{ProviderConfig, ProviderSettings};
/// Aggregated startup/session context.
pub use crate::context::SessionContext;
/// Public runtime/provider/tool error types.
pub use crate::errors::{AgentError, ProviderError, ToolContextError};
/// Event enum, event context, and dispatcher.
pub use crate::events::{Event, EventContext, EventManager};
/// File reference detection/listing/resolution helpers.
pub use crate::file_refs::{has_file_references, list_file_references, resolve_file_references};
/// Filesystem mode enum.
pub use crate::fs_mode::FsMode;
/// Hook APIs and callback signatures.
pub use crate::hooks::{ApprovalCallback, Hook, HookExecutor, HookRegistry, PromptCallback};
/// Observation event type.
pub use crate::observation::Observation;
/// Observation manager and persistence facade.
pub use crate::observation_manager::ObservationManager;
/// Core message and broker traits.
pub use crate::ports::{Message, MessageBroker};
/// Observation and plugin execution ports.
pub use crate::ports::{ObservationStore, PluginExecutor};
/// Provider override settings and provider factory.
pub use crate::providers::{ProviderOverrides, create_provider_with_overrides};
/// Rule schema and registry.
pub use crate::rules::{Rule, RuleRegistry};
/// Skill schema and registry.
pub use crate::skills::{Skill, SkillRegistry};
/// Shared typed IDs.
pub use crate::types::{ModelId, ToolId, ToolName};
/// AI analysis domain types from `looprs-core`.
pub use looprs_core::ai_types;
/// `/models` overview types and render/build entry points.
pub use model_catalog::{ModelsOverview, build_models_overview, render_models_overview};
