//! Coordinates multi-agent delegation, broker events, artifact sharing, and cancellation.

use std::collections::{HashMap, HashSet, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use futures::stream::{FuturesUnordered, StreamExt};
use looprs_core::api::{ContentBlock, Message as ChatMessage};
use looprs_core::ports::{Message as BrokerMessage, MessageBroker};
use serde::{Deserialize, Serialize};
use tokio::sync::watch;
use tokio::time::timeout;

use crate::agents::AgentRegistry;
use crate::app_config::AgentsConfig;
use crate::fs_mode::FsMode;
use crate::skills::{Skill, SkillRegistry};

/// Schema version used by all delegated-agent broker payloads.
pub const DELEGATION_SCHEMA_VERSION: u32 = 1;
/// Topic emitted immediately before a delegated agent loop starts.
pub const DELEGATION_STARTED_TOPIC_V1: &str = "looprs.agents.delegation.started.v1";
/// Topic emitted for every terminal delegated-agent result.
pub const DELEGATION_FINISHED_TOPIC_V1: &str = "looprs.agents.delegation.finished.v1";

/// One artifact explicitly returned by a delegated agent for optional sharing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegatedArtifact {
    /// Stable artifact name within the orchestration run.
    pub name: String,
    /// Text content made available to dependent agents.
    pub content: String,
}

/// Output from one independent delegated agent loop.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegatedAgentOutput {
    /// Final assistant message produced by the delegated loop.
    pub message: String,
    /// Explicit artifacts produced by the delegated loop.
    pub artifacts: Vec<DelegatedArtifact>,
}

/// A unit of delegated work and its dependency edges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelegatedTask {
    /// Stable task identifier used by dependency edges and result aggregation.
    pub id: String,
    /// Registered specialized agent to execute this task.
    pub agent_name: String,
    /// User prompt supplied only to this delegated history.
    pub prompt: String,
    /// Task identifiers that must succeed before this task can run.
    pub dependencies: Vec<String>,
}

impl DelegatedTask {
    /// Construct a dependency-free delegated task.
    pub fn new(
        id: impl Into<String>,
        agent_name: impl Into<String>,
        prompt: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            agent_name: agent_name.into(),
            prompt: prompt.into(),
            dependencies: Vec::new(),
        }
    }

    /// Add dependency identifiers in deterministic caller-provided order.
    pub fn with_dependencies(
        mut self,
        dependencies: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.dependencies = dependencies.into_iter().map(Into::into).collect();
        self
    }
}

/// Isolated capabilities and conversation state for one delegated loop.
#[derive(Debug, Clone)]
pub struct DelegatedAgentContext {
    /// Fresh history containing only this task and explicitly shared dependency results.
    pub history: Vec<ChatMessage>,
    /// Tool allowlist copied from the selected agent definition.
    pub tools: Vec<String>,
    /// Skill definitions copied from the selected agent definition.
    pub skills: Vec<Skill>,
    /// Filesystem policy applied to this delegated loop.
    pub fs_mode: FsMode,
    /// Explicit artifacts shared from successful direct dependencies.
    pub artifacts: Vec<DelegatedArtifact>,
}

/// Port that executes one isolated delegated agent loop.
#[async_trait::async_trait]
pub trait DelegatedAgentRunner: Send + Sync {
    /// Execute `task` using only the supplied isolated context.
    async fn run(
        &self,
        task: DelegatedTask,
        context: DelegatedAgentContext,
    ) -> Result<DelegatedAgentOutput, String>;
}

/// Terminal state for one delegated task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DelegationStatus {
    /// The delegated loop completed successfully.
    Succeeded,
    /// The delegated loop returned an error.
    Failed,
    /// The configured per-agent timeout elapsed.
    TimedOut,
    /// The orchestration run was cancelled.
    Cancelled,
    /// A dependency failed, timed out, was cancelled, or was skipped.
    Skipped,
}

impl DelegationStatus {
    /// Whether this terminal state permits dependent tasks to run.
    pub fn is_success(self) -> bool {
        self == Self::Succeeded
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::TimedOut => "timed_out",
            Self::Cancelled => "cancelled",
            Self::Skipped => "skipped",
        }
    }
}

/// Deterministically aggregated result for one delegated task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegationResult {
    /// Task identifier supplied in the plan.
    pub task_id: String,
    /// Specialized agent selected for the task.
    pub agent_name: String,
    /// Terminal task status.
    pub status: DelegationStatus,
    /// Successful output, when available.
    pub output: Option<DelegatedAgentOutput>,
    /// Failure, timeout, cancellation, or skip reason.
    pub error: Option<String>,
}

/// Results in the same order as the input tasks, independent of completion order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegationReport {
    /// One terminal result per input task.
    pub results: Vec<DelegationResult>,
}

/// Cooperative cancellation signal shared with one orchestration run.
#[derive(Debug, Clone)]
pub struct DelegationCancellation {
    sender: watch::Sender<bool>,
}

impl Default for DelegationCancellation {
    fn default() -> Self {
        let (sender, _) = watch::channel(false);
        Self { sender }
    }
}

impl DelegationCancellation {
    /// Create a signal in the active state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Cancel pending and active delegated loops.
    pub fn cancel(&self) {
        self.sender.send_replace(true);
    }

    /// Whether cancellation has already been requested.
    pub fn is_cancelled(&self) -> bool {
        *self.sender.borrow()
    }

    async fn cancelled(&self) {
        let mut receiver = self.sender.subscribe();
        let _ = receiver.wait_for(|cancelled| *cancelled).await;
    }
}

/// Invalid orchestration plans or runtime configuration.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum OrchestrationError {
    /// Concurrent execution requires a positive bound.
    #[error("agents.max_parallel must be greater than zero")]
    InvalidParallelism,
    /// Task identifiers must be unique and non-empty.
    #[error("duplicate or empty delegated task id: {0}")]
    InvalidTaskId(String),
    /// Every dependency must identify another task in the same plan.
    #[error("task '{task}' depends on unknown task '{dependency}'")]
    UnknownDependency {
        /// Task containing the invalid edge.
        task: String,
        /// Missing dependency identifier.
        dependency: String,
    },
    /// Every task must select a loaded specialized agent.
    #[error("task '{task}' selects unknown agent '{agent}'")]
    UnknownAgent {
        /// Task containing the invalid agent reference.
        task: String,
        /// Missing agent identifier.
        agent: String,
    },
    /// Every skill declared by an agent must be loaded before execution.
    #[error("agent '{agent}' requires missing skill '{skill}'")]
    MissingSkill {
        /// Agent containing the invalid skill reference.
        agent: String,
        /// Missing skill identifier.
        skill: String,
    },
    /// Dependency cycles cannot be scheduled.
    #[error("delegated task dependencies contain a cycle")]
    DependencyCycle,
}

type ExecutionFuture = Pin<Box<dyn Future<Output = (usize, ExecutionOutcome)> + Send + 'static>>;

enum ExecutionOutcome {
    Completed(Result<DelegatedAgentOutput, String>),
    TimedOut,
}

/// Dependency-aware, broker-observable coordinator for independent agent loops.
pub struct BrokerAgentOrchestrator {
    config: AgentsConfig,
    agents: AgentRegistry,
    skills: SkillRegistry,
    broker: Arc<dyn MessageBroker>,
    runner: Arc<dyn DelegatedAgentRunner>,
}

impl BrokerAgentOrchestrator {
    /// Construct an orchestrator from runtime registries and injected ports.
    pub fn new(
        config: AgentsConfig,
        agents: AgentRegistry,
        skills: SkillRegistry,
        broker: Arc<dyn MessageBroker>,
        runner: Arc<dyn DelegatedAgentRunner>,
    ) -> Self {
        Self {
            config,
            agents,
            skills,
            broker,
            runner,
        }
    }

    /// Execute a dependency graph with bounded concurrency and deterministic aggregation.
    pub async fn run(
        &self,
        tasks: Vec<DelegatedTask>,
        cancellation: DelegationCancellation,
    ) -> Result<DelegationReport, OrchestrationError> {
        // TODO(feature-idea 10): Expose multi-agent plans through public surfaces. (#57)
        // Support dependency-aware plans through the CLI and machine protocol.
        self.validate(&tasks)?;
        let run_id = uuid::Uuid::new_v4().to_string();
        let indexes = tasks
            .iter()
            .enumerate()
            .map(|(index, task)| (task.id.clone(), index))
            .collect::<HashMap<_, _>>();
        let mut results = vec![None; tasks.len()];
        let mut pending = (0..tasks.len()).collect::<HashSet<_>>();
        let mut in_flight = FuturesUnordered::<ExecutionFuture>::new();

        if cancellation.is_cancelled() {
            self.cancel_unfinished(&run_id, &tasks, &mut results);
            return Ok(aggregate_results(results));
        }

        while results.iter().any(Option::is_none) {
            self.skip_blocked_dependencies(&run_id, &tasks, &indexes, &mut pending, &mut results);
            self.schedule_ready(
                &run_id,
                &tasks,
                &indexes,
                &results,
                &mut pending,
                &mut in_flight,
            )?;

            if in_flight.is_empty() {
                break;
            }

            tokio::select! {
                _ = cancellation.cancelled() => {
                    drop(in_flight);
                    self.cancel_unfinished(&run_id, &tasks, &mut results);
                    return Ok(aggregate_results(results));
                }
                completed = in_flight.next() => {
                    if let Some((index, outcome)) = completed {
                        let result = result_from_outcome(&tasks[index], outcome);
                        self.publish_finished(&run_id, &result);
                        results[index] = Some(result);
                    }
                }
            }
        }

        Ok(aggregate_results(results))
    }

    fn validate(&self, tasks: &[DelegatedTask]) -> Result<(), OrchestrationError> {
        if self.config.max_parallel == 0 {
            return Err(OrchestrationError::InvalidParallelism);
        }
        let mut ids = HashSet::new();
        for task in tasks {
            if task.id.is_empty() || !ids.insert(task.id.as_str()) {
                return Err(OrchestrationError::InvalidTaskId(task.id.clone()));
            }
            let Some(agent) = self.agents.get(&task.agent_name) else {
                return Err(OrchestrationError::UnknownAgent {
                    task: task.id.clone(),
                    agent: task.agent_name.clone(),
                });
            };
            for skill in &agent.skills {
                if self.skills.get(skill).is_none() {
                    return Err(OrchestrationError::MissingSkill {
                        agent: agent.name.clone(),
                        skill: skill.clone(),
                    });
                }
            }
        }
        for task in tasks {
            for dependency in &task.dependencies {
                if !ids.contains(dependency.as_str()) {
                    return Err(OrchestrationError::UnknownDependency {
                        task: task.id.clone(),
                        dependency: dependency.clone(),
                    });
                }
            }
        }
        validate_acyclic(tasks)
    }

    fn schedule_ready(
        &self,
        run_id: &str,
        tasks: &[DelegatedTask],
        indexes: &HashMap<String, usize>,
        results: &[Option<DelegationResult>],
        pending: &mut HashSet<usize>,
        in_flight: &mut FuturesUnordered<ExecutionFuture>,
    ) -> Result<(), OrchestrationError> {
        while in_flight.len() < self.config.max_parallel {
            let Some(index) = pending
                .iter()
                .copied()
                .filter(|index| dependencies_succeeded(&tasks[*index], indexes, results))
                .min()
            else {
                break;
            };
            pending.remove(&index);
            let task = tasks[index].clone();
            let context = self.context_for(&task, indexes, results)?;
            self.publish_started(run_id, &task);
            let runner = Arc::clone(&self.runner);
            let timeout_seconds = self.config.timeout_seconds;
            in_flight.push(Box::pin(async move {
                let execution = runner.run(task, context);
                let outcome = match timeout_seconds {
                    Some(seconds) => match timeout(Duration::from_secs(seconds), execution).await {
                        Ok(result) => ExecutionOutcome::Completed(result),
                        Err(_) => ExecutionOutcome::TimedOut,
                    },
                    None => ExecutionOutcome::Completed(execution.await),
                };
                (index, outcome)
            }));
        }
        Ok(())
    }

    fn context_for(
        &self,
        task: &DelegatedTask,
        indexes: &HashMap<String, usize>,
        results: &[Option<DelegationResult>],
    ) -> Result<DelegatedAgentContext, OrchestrationError> {
        let agent =
            self.agents
                .get(&task.agent_name)
                .ok_or_else(|| OrchestrationError::UnknownAgent {
                    task: task.id.clone(),
                    agent: task.agent_name.clone(),
                })?;
        let skills = agent
            .skills
            .iter()
            .map(|name| {
                self.skills
                    .get(name)
                    .cloned()
                    .ok_or_else(|| OrchestrationError::MissingSkill {
                        agent: agent.name.clone(),
                        skill: name.clone(),
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut history = Vec::new();
        let mut artifacts = Vec::new();
        if self.config.context_sharing {
            for dependency in &task.dependencies {
                if let Some(result) = &results[indexes[dependency]]
                    && let Some(output) = &result.output
                {
                    history.push(ChatMessage::assistant(vec![ContentBlock::Text {
                        text: output.message.clone(),
                    }]));
                    artifacts.extend(output.artifacts.clone());
                }
            }
        }
        history.push(ChatMessage::user(task.prompt.clone()));
        Ok(DelegatedAgentContext {
            history,
            tools: agent.tools.clone(),
            skills,
            fs_mode: self.config.fs_mode,
            artifacts,
        })
    }

    fn skip_blocked_dependencies(
        &self,
        run_id: &str,
        tasks: &[DelegatedTask],
        indexes: &HashMap<String, usize>,
        pending: &mut HashSet<usize>,
        results: &mut [Option<DelegationResult>],
    ) {
        loop {
            let blocked = pending
                .iter()
                .copied()
                .filter(|index| dependencies_finished(&tasks[*index], indexes, results))
                .filter(|index| !dependencies_succeeded(&tasks[*index], indexes, results))
                .min();
            let Some(index) = blocked else {
                break;
            };
            pending.remove(&index);
            let result = terminal_result(
                &tasks[index],
                DelegationStatus::Skipped,
                None,
                Some("dependency did not succeed".to_string()),
            );
            self.publish_finished(run_id, &result);
            results[index] = Some(result);
        }
    }

    fn cancel_unfinished(
        &self,
        run_id: &str,
        tasks: &[DelegatedTask],
        results: &mut [Option<DelegationResult>],
    ) {
        for (index, task) in tasks.iter().enumerate() {
            if results[index].is_none() {
                let result = terminal_result(
                    task,
                    DelegationStatus::Cancelled,
                    None,
                    Some("orchestration cancelled".to_string()),
                );
                self.publish_finished(run_id, &result);
                results[index] = Some(result);
            }
        }
    }

    fn publish_started(&self, run_id: &str, task: &DelegatedTask) {
        self.broker.publish(BrokerMessage::new(
            "looprs.orchestrator",
            DELEGATION_STARTED_TOPIC_V1,
            DELEGATION_SCHEMA_VERSION,
            serde_json::json!({
                "run_id": run_id,
                "task_id": task.id,
                "agent_name": task.agent_name,
            }),
        ));
    }

    fn publish_finished(&self, run_id: &str, result: &DelegationResult) {
        self.broker.publish(BrokerMessage::new(
            "looprs.orchestrator",
            DELEGATION_FINISHED_TOPIC_V1,
            DELEGATION_SCHEMA_VERSION,
            serde_json::json!({
                "run_id": run_id,
                "task_id": result.task_id,
                "agent_name": result.agent_name,
                "status": result.status.as_str(),
                "output": result.output,
                "error": result.error,
            }),
        ));
    }
}

fn dependencies_finished(
    task: &DelegatedTask,
    indexes: &HashMap<String, usize>,
    results: &[Option<DelegationResult>],
) -> bool {
    task.dependencies
        .iter()
        .all(|dependency| results[indexes[dependency]].is_some())
}

fn dependencies_succeeded(
    task: &DelegatedTask,
    indexes: &HashMap<String, usize>,
    results: &[Option<DelegationResult>],
) -> bool {
    task.dependencies.iter().all(|dependency| {
        results[indexes[dependency]]
            .as_ref()
            .is_some_and(|result| result.status.is_success())
    })
}

fn validate_acyclic(tasks: &[DelegatedTask]) -> Result<(), OrchestrationError> {
    let mut indegree = tasks
        .iter()
        .map(|task| (task.id.as_str(), task.dependencies.len()))
        .collect::<HashMap<_, _>>();
    let mut dependents = HashMap::<&str, Vec<&str>>::new();
    for task in tasks {
        for dependency in &task.dependencies {
            dependents
                .entry(dependency.as_str())
                .or_default()
                .push(task.id.as_str());
        }
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(id, degree)| (*degree == 0).then_some(*id))
        .collect::<VecDeque<_>>();
    let mut visited = 0;
    while let Some(id) = ready.pop_front() {
        visited += 1;
        for dependent in dependents.get(id).into_iter().flatten() {
            if let Some(degree) = indegree.get_mut(dependent) {
                *degree -= 1;
                if *degree == 0 {
                    ready.push_back(dependent);
                }
            }
        }
    }
    if visited == tasks.len() {
        Ok(())
    } else {
        Err(OrchestrationError::DependencyCycle)
    }
}

fn result_from_outcome(task: &DelegatedTask, outcome: ExecutionOutcome) -> DelegationResult {
    match outcome {
        ExecutionOutcome::Completed(Ok(output)) => {
            terminal_result(task, DelegationStatus::Succeeded, Some(output), None)
        }
        ExecutionOutcome::Completed(Err(error)) => {
            terminal_result(task, DelegationStatus::Failed, None, Some(error))
        }
        ExecutionOutcome::TimedOut => terminal_result(
            task,
            DelegationStatus::TimedOut,
            None,
            Some("delegated agent timed out".to_string()),
        ),
    }
}

fn terminal_result(
    task: &DelegatedTask,
    status: DelegationStatus,
    output: Option<DelegatedAgentOutput>,
    error: Option<String>,
) -> DelegationResult {
    DelegationResult {
        task_id: task.id.clone(),
        agent_name: task.agent_name.clone(),
        status,
        output,
        error,
    }
}

fn aggregate_results(results: Vec<Option<DelegationResult>>) -> DelegationReport {
    DelegationReport {
        results: results.into_iter().flatten().collect(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use looprs_core::adapters::ChannelBroker;
    use looprs_core::ports::MessageBroker;

    use crate::agents::{AgentDefinition, AgentRegistry};
    use crate::app_config::AgentsConfig;
    use crate::fs_mode::FsMode;
    use crate::skills::{Skill, SkillRegistry};

    use super::{
        BrokerAgentOrchestrator, DELEGATION_FINISHED_TOPIC_V1, DELEGATION_SCHEMA_VERSION,
        DELEGATION_STARTED_TOPIC_V1, DelegatedAgentContext, DelegatedAgentOutput,
        DelegatedAgentRunner, DelegatedArtifact, DelegatedTask, DelegationCancellation,
        DelegationStatus,
    };

    #[derive(Default)]
    struct RecordingRunner {
        active: AtomicUsize,
        max_active: AtomicUsize,
        contexts: Mutex<HashMap<String, DelegatedAgentContext>>,
        delays: HashMap<String, Duration>,
        failures: HashSet<String>,
    }

    #[async_trait::async_trait]
    impl DelegatedAgentRunner for RecordingRunner {
        async fn run(
            &self,
            task: DelegatedTask,
            context: DelegatedAgentContext,
        ) -> Result<DelegatedAgentOutput, String> {
            self.contexts
                .lock()
                .expect("contexts lock")
                .insert(task.id.clone(), context);
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.max_active.fetch_max(active, Ordering::SeqCst);
            if let Some(delay) = self.delays.get(&task.id) {
                tokio::time::sleep(*delay).await;
            }
            self.active.fetch_sub(1, Ordering::SeqCst);
            if self.failures.contains(&task.id) {
                return Err(format!("{} failed", task.id));
            }
            Ok(DelegatedAgentOutput {
                message: format!("{} result", task.id),
                artifacts: vec![DelegatedArtifact {
                    name: format!("{}.txt", task.id),
                    content: format!("{} artifact", task.id),
                }],
            })
        }
    }

    fn agent(name: &str, tools: &[&str], skills: &[&str]) -> AgentDefinition {
        AgentDefinition {
            name: name.to_string(),
            role: None,
            description: None,
            system_prompt: None,
            tools: tools.iter().map(|value| (*value).to_string()).collect(),
            skills: skills.iter().map(|value| (*value).to_string()).collect(),
            constraints: Vec::new(),
            triggers: Vec::new(),
        }
    }

    fn registries() -> (AgentRegistry, SkillRegistry) {
        let mut agents = AgentRegistry::new();
        agents.register(agent("planner", &["read"], &["planning"]));
        agents.register(agent("reviewer", &["read", "grep"], &["reviewing"]));
        let mut skills = SkillRegistry::new();
        for name in ["planning", "reviewing"] {
            skills.register(Skill {
                name: name.to_string(),
                description: None,
                triggers: Vec::new(),
                content: format!("{name} instructions"),
                source_path: PathBuf::from(format!("/{name}/SKILL.md")),
            });
        }
        (agents, skills)
    }

    fn orchestrator(
        config: AgentsConfig,
        runner: Arc<RecordingRunner>,
        broker: Arc<ChannelBroker>,
    ) -> BrokerAgentOrchestrator {
        let (agents, skills) = registries();
        BrokerAgentOrchestrator::new(config, agents, skills, broker, runner)
    }

    #[tokio::test]
    async fn delegated_contexts_are_isolated_and_sharing_is_explicit() {
        for context_sharing in [false, true] {
            let runner = Arc::new(RecordingRunner::default());
            let broker = Arc::new(ChannelBroker::new());
            let config = AgentsConfig {
                context_sharing,
                fs_mode: FsMode::Read,
                ..AgentsConfig::default()
            };
            let service = orchestrator(config, Arc::clone(&runner), broker);
            let tasks = vec![
                DelegatedTask::new("plan", "planner", "make a plan"),
                DelegatedTask::new("review", "reviewer", "review it").with_dependencies(["plan"]),
            ];

            let report = service
                .run(tasks, DelegationCancellation::new())
                .await
                .expect("orchestration succeeds");

            assert!(
                report
                    .results
                    .iter()
                    .all(|result| result.status.is_success())
            );
            let contexts = runner.contexts.lock().expect("contexts lock");
            let plan = contexts.get("plan").expect("plan context");
            let review = contexts.get("review").expect("review context");
            assert_eq!(plan.tools, vec!["read"]);
            assert_eq!(review.tools, vec!["read", "grep"]);
            assert_eq!(plan.skills[0].name, "planning");
            assert_eq!(review.skills[0].name, "reviewing");
            assert_eq!(plan.fs_mode, FsMode::Read);
            assert_eq!(plan.history.len(), 1);
            assert_eq!(review.history.len(), if context_sharing { 2 } else { 1 });
            assert_eq!(review.artifacts.len(), if context_sharing { 1 } else { 0 });
        }
    }

    #[tokio::test]
    async fn max_parallel_is_bounded_and_results_are_deterministic() {
        let runner = Arc::new(RecordingRunner {
            delays: HashMap::from([
                ("slow".to_string(), Duration::from_millis(60)),
                ("fast".to_string(), Duration::from_millis(5)),
                ("middle".to_string(), Duration::from_millis(20)),
            ]),
            ..RecordingRunner::default()
        });
        let broker = Arc::new(ChannelBroker::new());
        let service = orchestrator(
            AgentsConfig {
                max_parallel: 2,
                ..AgentsConfig::default()
            },
            Arc::clone(&runner),
            broker,
        );
        let tasks = ["slow", "fast", "middle"]
            .map(|id| DelegatedTask::new(id, "planner", id))
            .to_vec();

        let report = service
            .run(tasks, DelegationCancellation::new())
            .await
            .expect("orchestration succeeds");

        assert_eq!(runner.max_active.load(Ordering::SeqCst), 2);
        assert_eq!(
            report
                .results
                .iter()
                .map(|result| result.task_id.as_str())
                .collect::<Vec<_>>(),
            vec!["slow", "fast", "middle"]
        );
    }

    #[tokio::test]
    async fn partial_failure_skips_dependents_but_keeps_independent_results() {
        let runner = Arc::new(RecordingRunner {
            failures: HashSet::from(["broken".to_string()]),
            ..RecordingRunner::default()
        });
        let service = orchestrator(
            AgentsConfig::default(),
            runner,
            Arc::new(ChannelBroker::new()),
        );
        let tasks = vec![
            DelegatedTask::new("broken", "planner", "fail"),
            DelegatedTask::new("dependent", "reviewer", "blocked").with_dependencies(["broken"]),
            DelegatedTask::new("independent", "reviewer", "continue"),
        ];

        let report = service
            .run(tasks, DelegationCancellation::new())
            .await
            .expect("partial failures produce a report");

        assert_eq!(report.results[0].status, DelegationStatus::Failed);
        assert_eq!(report.results[1].status, DelegationStatus::Skipped);
        assert_eq!(report.results[2].status, DelegationStatus::Succeeded);
    }

    #[tokio::test]
    async fn timeout_and_cancellation_are_terminal_results() {
        let runner = Arc::new(RecordingRunner {
            delays: HashMap::from([("slow".to_string(), Duration::from_millis(50))]),
            ..RecordingRunner::default()
        });
        let service = orchestrator(
            AgentsConfig {
                timeout_seconds: Some(0),
                ..AgentsConfig::default()
            },
            runner,
            Arc::new(ChannelBroker::new()),
        );
        let timed_out = service
            .run(
                vec![DelegatedTask::new("slow", "planner", "wait")],
                DelegationCancellation::new(),
            )
            .await
            .expect("timeout report");
        assert_eq!(timed_out.results[0].status, DelegationStatus::TimedOut);

        let cancellation = DelegationCancellation::new();
        cancellation.cancel();
        let cancelled = service
            .run(
                vec![DelegatedTask::new("never", "planner", "stop")],
                cancellation,
            )
            .await
            .expect("cancellation report");
        assert_eq!(cancelled.results[0].status, DelegationStatus::Cancelled);
    }

    #[tokio::test]
    async fn cancellation_stops_an_active_delegated_loop() {
        let runner = Arc::new(RecordingRunner {
            delays: HashMap::from([("active".to_string(), Duration::from_secs(5))]),
            ..RecordingRunner::default()
        });
        let service = orchestrator(
            AgentsConfig {
                timeout_seconds: None,
                ..AgentsConfig::default()
            },
            Arc::clone(&runner),
            Arc::new(ChannelBroker::new()),
        );
        let cancellation = DelegationCancellation::new();
        let task_cancellation = cancellation.clone();
        let handle = tokio::spawn(async move {
            service
                .run(
                    vec![DelegatedTask::new("active", "planner", "wait")],
                    task_cancellation,
                )
                .await
        });
        while runner.active.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }

        cancellation.cancel();
        let report = handle
            .await
            .expect("orchestration task joins")
            .expect("cancellation report");

        assert_eq!(report.results[0].status, DelegationStatus::Cancelled);
    }

    #[tokio::test]
    async fn broker_events_use_versioned_observable_topics_and_schema() {
        let runner = Arc::new(RecordingRunner::default());
        let broker = Arc::new(ChannelBroker::new());
        let mut started = broker.subscribe(DELEGATION_STARTED_TOPIC_V1);
        let mut finished = broker.subscribe(DELEGATION_FINISHED_TOPIC_V1);
        let service = orchestrator(AgentsConfig::default(), runner, Arc::clone(&broker));

        service
            .run(
                vec![DelegatedTask::new("plan", "planner", "plan")],
                DelegationCancellation::new(),
            )
            .await
            .expect("orchestration succeeds");

        let started = started.try_recv().expect("started event");
        let finished = finished.try_recv().expect("finished event");
        assert_eq!(started.topic, DELEGATION_STARTED_TOPIC_V1);
        assert_eq!(finished.topic, DELEGATION_FINISHED_TOPIC_V1);
        assert_eq!(started.schema_version, DELEGATION_SCHEMA_VERSION);
        assert_eq!(finished.schema_version, DELEGATION_SCHEMA_VERSION);
        assert_eq!(started.payload["task_id"], "plan");
        assert_eq!(finished.payload["status"], "succeeded");
    }
}
