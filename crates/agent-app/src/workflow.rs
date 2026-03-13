use crate::agents::{Agent, ImpactAnalyzer, PlannerAgent, ReaderAgent, RootCauseAnalyzer};
use agent_domain::{
    ImpactAnalysis, Incident, IncidentFacts, JiraPort, LlmPort, RemediationPlan,
    RootCauseAnalysis, TracePort,
};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// IncidentUseCase — orchestrates the full multi-agent workflow
//
//   Incident
//     │
//     ▼
//   ReaderAgent  (extract facts)
//     │
//     ├──────────────────┐
//     ▼                  ▼
//   ImpactAnalyzer    RootCauseAnalyzer   ← parallel via tokio::try_join!
//     │                  │
//     └──────┬───────────┘
//            ▼
//        PlannerAgent  (remediation + optional Jira ticket)
//
// ---------------------------------------------------------------------------

pub struct IncidentUseCase<'a> {
    reader: ReaderAgent<'a>,
    impact_analyzer: ImpactAnalyzer<'a>,
    rca_analyzer: RootCauseAnalyzer<'a>,
    planner: PlannerAgent<'a>,
    trace: &'a dyn TracePort,
}

impl<'a> IncidentUseCase<'a> {
    pub fn new(
        llm: &'a dyn LlmPort,
        jira: &'a dyn JiraPort,
        trace: &'a dyn TracePort,
    ) -> Self {
        Self {
            reader: ReaderAgent::new(llm),
            impact_analyzer: ImpactAnalyzer::new(llm),
            rca_analyzer: RootCauseAnalyzer::new(llm),
            planner: PlannerAgent::new(llm, jira),
            trace,
        }
    }

    pub async fn execute(
        &self,
        incident: Incident,
    ) -> anyhow::Result<(String, RemediationPlan)> {
        let trace_id = Uuid::new_v4().to_string();

        // Phase 1: extract structured facts
        let facts = self
            .reader
            .run(incident, &trace_id, self.trace)
            .await?;

        // Phase 2: parallel analysis — impact + root-cause run concurrently
        let (impact, rca) = Self::parallel_analyze(
            &self.impact_analyzer,
            &self.rca_analyzer,
            facts.clone(),
            &trace_id,
            self.trace,
        )
        .await?;

        // Phase 3: plan remediation (may call Jira)
        let plan = self
            .planner
            .run((facts, impact, rca), &trace_id, self.trace)
            .await?;

        Ok((trace_id, plan))
    }

    /// Run impact and root-cause analyzers in parallel via `tokio::try_join!`.
    async fn parallel_analyze(
        impact_analyzer: &ImpactAnalyzer<'_>,
        rca_analyzer: &RootCauseAnalyzer<'_>,
        facts: IncidentFacts,
        trace_id: &str,
        trace: &dyn TracePort,
    ) -> anyhow::Result<(ImpactAnalysis, RootCauseAnalysis)> {
        let impact_facts = facts.clone();
        let rca_facts = facts;

        let impact_fut = impact_analyzer.run(impact_facts, trace_id, trace);
        let rca_fut = rca_analyzer.run(rca_facts, trace_id, trace);

        let (impact, rca) = tokio::try_join!(impact_fut, rca_fut)?;

        Ok((impact, rca))
    }
}
