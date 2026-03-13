use agent_domain::{
    ImpactAnalysis, Incident, IncidentFacts, JiraPort, LlmPort, PlanStep, RemediationPlan,
    RootCauseAnalysis, StepKind, TracePort, TraceStep,
};
use async_trait::async_trait;
use chrono::Utc;

// ---------------------------------------------------------------------------
// Generic agent trait
// ---------------------------------------------------------------------------

#[async_trait]
pub trait Agent<I, O>: Send + Sync {
    async fn run(&self, input: I, trace_id: &str, trace: &dyn TracePort) -> anyhow::Result<O>;
}

/// Parse a `serde_json::Value` into a concrete type.
fn parse_json<T: serde::de::DeserializeOwned>(val: serde_json::Value) -> anyhow::Result<T> {
    serde_json::from_value(val).map_err(|e| anyhow::anyhow!("JSON parse error: {e}"))
}

// ---------------------------------------------------------------------------
// Reader agent — extracts structured facts from raw incident text
// ---------------------------------------------------------------------------

pub struct ReaderAgent<'a> {
    llm: &'a dyn LlmPort,
}

impl<'a> ReaderAgent<'a> {
    pub fn new(llm: &'a dyn LlmPort) -> Self {
        Self { llm }
    }
}

#[derive(Debug, serde::Deserialize)]
struct ReaderOut {
    service: String,
    symptoms: String,
    timeline: String,
}

#[async_trait]
impl Agent<Incident, IncidentFacts> for ReaderAgent<'_> {
    async fn run(
        &self,
        incident: Incident,
        trace_id: &str,
        trace: &dyn TracePort,
    ) -> anyhow::Result<IncidentFacts> {
        trace
            .append_step(TraceStep {
                trace_id: trace_id.to_string(),
                step_index: 0,
                kind: StepKind::Intent,
                timestamp_iso: Utc::now().to_rfc3339(),
                label: format!("Incident: {}", incident.title),
                parent_step_index: None,
                raw_snapshot: Some(incident.raw_text.clone()),
                metadata_json: serde_json::json!({}),
            })
            .await?;

        let system = "You extract structured incident facts. \
                       Respond only with JSON containing keys: service, symptoms, timeline.";
        let user = format!(
            "Extract service, symptoms, timeline as JSON.\nIncident:\n{}",
            incident.raw_text
        );

        let val = self.llm.complete_json(system, &user).await?;
        let out: ReaderOut = parse_json(val)?;

        let facts = IncidentFacts {
            service: out.service,
            symptoms: out.symptoms,
            timeline: out.timeline,
        };

        trace
            .append_step(TraceStep {
                trace_id: trace_id.to_string(),
                step_index: 1,
                kind: StepKind::Observation,
                timestamp_iso: Utc::now().to_rfc3339(),
                label: "Reader extracted facts".into(),
                parent_step_index: Some(0),
                raw_snapshot: None,
                metadata_json: serde_json::to_value(&facts)?,
            })
            .await?;

        Ok(facts)
    }
}

// ---------------------------------------------------------------------------
// Impact analyzer — determines severity and customer impact
// ---------------------------------------------------------------------------

pub struct ImpactAnalyzer<'a> {
    llm: &'a dyn LlmPort,
}

impl<'a> ImpactAnalyzer<'a> {
    pub fn new(llm: &'a dyn LlmPort) -> Self {
        Self { llm }
    }
}

#[derive(Debug, serde::Deserialize)]
struct ImpactOut {
    severity: String,
    customer_impact: String,
}

#[async_trait]
impl Agent<IncidentFacts, ImpactAnalysis> for ImpactAnalyzer<'_> {
    async fn run(
        &self,
        facts: IncidentFacts,
        trace_id: &str,
        trace: &dyn TracePort,
    ) -> anyhow::Result<ImpactAnalysis> {
        let step_index = 10; // impact branch

        trace
            .append_step(TraceStep {
                trace_id: trace_id.to_string(),
                step_index,
                kind: StepKind::Thought,
                timestamp_iso: Utc::now().to_rfc3339(),
                label: "Analyze impact".into(),
                parent_step_index: Some(1),
                raw_snapshot: None,
                metadata_json: serde_json::to_value(&facts)?,
            })
            .await?;

        let system = "You are an SRE impact analyzer. \
                       Respond only with JSON containing keys: severity, customer_impact.";
        let user = format!(
            "Given these incident facts, determine severity and customer impact as JSON.\n\
             Service: {}\nSymptoms: {}\nTimeline: {}",
            facts.service, facts.symptoms, facts.timeline
        );

        let val = self.llm.complete_json(system, &user).await?;
        let out: ImpactOut = parse_json(val)?;
        let analysis = ImpactAnalysis {
            severity: out.severity,
            customer_impact: out.customer_impact,
        };

        trace
            .append_step(TraceStep {
                trace_id: trace_id.to_string(),
                step_index: step_index + 1,
                kind: StepKind::Observation,
                timestamp_iso: Utc::now().to_rfc3339(),
                label: "Impact result".into(),
                parent_step_index: Some(step_index),
                raw_snapshot: None,
                metadata_json: serde_json::to_value(&analysis)?,
            })
            .await?;

        Ok(analysis)
    }
}

// ---------------------------------------------------------------------------
// Root-cause analyzer — runs in parallel with impact analyzer
// ---------------------------------------------------------------------------

pub struct RootCauseAnalyzer<'a> {
    llm: &'a dyn LlmPort,
}

impl<'a> RootCauseAnalyzer<'a> {
    pub fn new(llm: &'a dyn LlmPort) -> Self {
        Self { llm }
    }
}

#[derive(Debug, serde::Deserialize)]
struct RcaOut {
    cause: String,
    evidence: String,
    confidence: String,
}

#[async_trait]
impl Agent<IncidentFacts, RootCauseAnalysis> for RootCauseAnalyzer<'_> {
    async fn run(
        &self,
        facts: IncidentFacts,
        trace_id: &str,
        trace: &dyn TracePort,
    ) -> anyhow::Result<RootCauseAnalysis> {
        let step_index = 20; // rca branch

        trace
            .append_step(TraceStep {
                trace_id: trace_id.to_string(),
                step_index,
                kind: StepKind::Thought,
                timestamp_iso: Utc::now().to_rfc3339(),
                label: "Analyze root cause".into(),
                parent_step_index: Some(1),
                raw_snapshot: None,
                metadata_json: serde_json::to_value(&facts)?,
            })
            .await?;

        let system = "You are an SRE root-cause analyst. \
                       Respond only with JSON containing keys: cause, evidence, confidence.";
        let user = format!(
            "Given these incident facts, determine the most likely root cause as JSON.\n\
             Service: {}\nSymptoms: {}\nTimeline: {}",
            facts.service, facts.symptoms, facts.timeline
        );

        let val = self.llm.complete_json(system, &user).await?;
        let out: RcaOut = parse_json(val)?;
        let rca = RootCauseAnalysis {
            cause: out.cause,
            evidence: out.evidence,
            confidence: out.confidence,
        };

        trace
            .append_step(TraceStep {
                trace_id: trace_id.to_string(),
                step_index: step_index + 1,
                kind: StepKind::Observation,
                timestamp_iso: Utc::now().to_rfc3339(),
                label: "Root-cause result".into(),
                parent_step_index: Some(step_index),
                raw_snapshot: None,
                metadata_json: serde_json::to_value(&rca)?,
            })
            .await?;

        Ok(rca)
    }
}

// ---------------------------------------------------------------------------
// Planner agent — produces remediation plan, optionally creates Jira ticket
// ---------------------------------------------------------------------------

pub struct PlannerAgent<'a> {
    llm: &'a dyn LlmPort,
    jira: &'a dyn JiraPort,
}

impl<'a> PlannerAgent<'a> {
    pub fn new(llm: &'a dyn LlmPort, jira: &'a dyn JiraPort) -> Self {
        Self { llm, jira }
    }
}

#[derive(Debug, serde::Deserialize)]
struct PlannerOut {
    create_jira_issue: bool,
    project_key: Option<String>,
    issue_type: Option<String>,
    summary: Option<String>,
    description: Option<String>,
    steps: Vec<PlanStep>,
}

#[async_trait]
impl Agent<(IncidentFacts, ImpactAnalysis, RootCauseAnalysis), RemediationPlan>
    for PlannerAgent<'_>
{
    async fn run(
        &self,
        input: (IncidentFacts, ImpactAnalysis, RootCauseAnalysis),
        trace_id: &str,
        trace: &dyn TracePort,
    ) -> anyhow::Result<RemediationPlan> {
        let (facts, impact, rca) = input;
        let step_index = 30;

        trace
            .append_step(TraceStep {
                trace_id: trace_id.to_string(),
                step_index,
                kind: StepKind::Thought,
                timestamp_iso: Utc::now().to_rfc3339(),
                label: "Plan remediation".into(),
                parent_step_index: None,
                raw_snapshot: None,
                metadata_json: serde_json::json!({
                    "facts": facts,
                    "impact": impact,
                    "root_cause": rca,
                }),
            })
            .await?;

        let system = "You are an SRE planner. \
                       Respond with JSON containing: create_jira_issue (bool), \
                       project_key, issue_type, summary, description (all optional strings), \
                       steps (array of {action, owner, priority}).";
        let user = format!(
            "Produce a remediation plan and Jira decision as JSON.\n\
             Facts — Service: {}, Symptoms: {}, Timeline: {}\n\
             Impact — Severity: {}, Customer Impact: {}\n\
             Root Cause — {}: {} (confidence: {})",
            facts.service,
            facts.symptoms,
            facts.timeline,
            impact.severity,
            impact.customer_impact,
            rca.cause,
            rca.evidence,
            rca.confidence,
        );

        let val = self.llm.complete_json(system, &user).await?;
        let out: PlannerOut = parse_json(val)?;
        let mut plan = RemediationPlan { steps: out.steps };

        if out.create_jira_issue {
            let spec = agent_domain::JiraIssueSpec {
                project_key: out.project_key.unwrap_or_else(|| "ENG".into()),
                issue_type: out.issue_type.unwrap_or_else(|| "Task".into()),
                summary: out.summary.unwrap_or_else(|| facts.service.clone()),
                description: out.description.unwrap_or_else(|| {
                    format!(
                        "Service: {}\nSeverity: {}\nRoot Cause: {}",
                        facts.service, impact.severity, rca.cause,
                    )
                }),
            };

            let issue = self.jira.create_issue(spec).await?;

            plan.steps.insert(
                0,
                PlanStep {
                    action: format!("Track in Jira {}", issue.key),
                    owner: "oncall-sre".into(),
                    priority: "P0".into(),
                },
            );

            trace
                .append_step(TraceStep {
                    trace_id: trace_id.to_string(),
                    step_index: step_index + 1,
                    kind: StepKind::ToolCall,
                    timestamp_iso: Utc::now().to_rfc3339(),
                    label: format!("Created Jira {}", issue.key),
                    parent_step_index: Some(step_index),
                    raw_snapshot: None,
                    metadata_json: serde_json::to_value(&issue)?,
                })
                .await?;
        }

        trace
            .append_step(TraceStep {
                trace_id: trace_id.to_string(),
                step_index: step_index + 2,
                kind: StepKind::StateDelta,
                timestamp_iso: Utc::now().to_rfc3339(),
                label: "Remediation plan finalized".into(),
                parent_step_index: Some(step_index),
                raw_snapshot: None,
                metadata_json: serde_json::to_value(&plan)?,
            })
            .await?;

        Ok(plan)
    }
}
