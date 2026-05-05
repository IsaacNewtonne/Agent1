use crate::types::{ExecutionPlan, ExecutionStep, OrchestratorConfig, PlanView};
use agent1_core::{
    AgentRole, ChatMessage, ChatRequest,
    PlanId, PlanStatus, new_id, now, Agent1Error, ModelConfig, Result,
};
use agent1_models::provider_for;
use serde::Deserialize;
use std::collections::HashSet;

fn planner_model_config() -> ModelConfig {
    ModelConfig {
        provider: String::from("ollama"),
        model: String::from("llama3.1:8b"),
        base_url: None,
        context_window: 8192,
        temperature: 0.2,
        top_p: None,
        max_tokens: None,
    }
}

#[derive(Debug, Deserialize)]
struct PlanStep {
    description: String,
    dependencies: Vec<String>,
    assigned_role: String,
}

#[derive(Debug, Deserialize)]
struct GeneratedPlan {
    summary: String,
    steps: Vec<PlanStep>,
}

pub struct GoalDecomposer {
    config: OrchestratorConfig,
}

impl GoalDecomposer {
    pub fn new(config: OrchestratorConfig) -> Self {
        Self { config }
    }

    pub async fn decompose(
        &self,
        goal: &str,
        orchestration_id: &str,
    ) -> Result<PlanView> {
        let plan_id = new_id("plan");
        let plan = ExecutionPlan::new(
            orchestration_id.to_string(),
            format!("Plan for: {}", goal),
            goal.to_string(),
        );

        let prompt = self.build_decomposition_prompt(goal);
        let plan_json = self.call_planner(&prompt).await?;
        let generated = self.parse_plan_response(&plan_json)?;

        let steps = self.build_steps(&plan_id, generated.steps)?;

        let plan_with_status = ExecutionPlan {
            id: plan.id,
            orchestration_id: plan.orchestration_id,
            objective: plan.objective,
            raw_goal: plan.raw_goal,
            status: PlanStatus::Planned,
            created_at: now(),
            completed_at: None,
        };

        Ok(PlanView {
            plan: plan_with_status,
            steps,
        })
    }

    fn build_decomposition_prompt(&self, goal: &str) -> String {
        format!(
            r#"You are a Planner agent helping break down a complex objective into execution steps.

Objective: {}

Generate a detailed execution plan as JSON with this exact format:
{{
  "summary": "Brief summary of the plan approach",
  "steps": [
    {{
      "description": "Clear description of what this step does",
      "dependencies": ["list of step indices this depends on, e.g. [0] for first step"],
      "assigned_role": "planner|worker|critic|researcher|builder|reporter"
    }}
  ]
}}

Rules:
1. Break the goal into 3-10 focused steps
2. Each step should be independently actionable
3. List dependencies as array of step indices (0-based)
4. Steps without dependencies can run in parallel
5. Use "worker" for most execution tasks
6. Use "researcher" for information gathering
7. Use "builder" for creating artifacts (code, docs)
8. Use "critic" only for review/validation steps
9. Use "reporter" only for final summary/reporting
10. Keep descriptions concise but actionable

Return ONLY valid JSON, no markdown or explanation."#,
            goal
        )
    }

    async fn call_planner(&self, prompt: &str) -> Result<String> {
        let provider = provider_for(&planner_model_config())?;

        let request = ChatRequest {
            model: planner_model_config(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
        };

        let response = provider.chat(request).await?;
        Ok(response.content)
    }

    fn parse_plan_response(&self, content: &str) -> Result<GeneratedPlan> {
        let trimmed = content.trim();

        let cleaned = if trimmed.starts_with("```json") {
            trimmed
                .trim_start_matches("```json")
                .trim_end_matches("```")
                .trim()
        } else if trimmed.starts_with("```") {
            trimmed
                .trim_start_matches("```")
                .trim_end_matches("```")
                .trim()
        } else {
            trimmed
        };

        serde_json::from_str(cleaned).map_err(|err| {
            Agent1Error::InvalidModelResponse(format!("failed to parse plan JSON: {err}, content: {cleaned}"))
        })
    }

    fn build_steps(
        &self,
        plan_id: &PlanId,
        generated_steps: Vec<PlanStep>,
    ) -> Result<Vec<ExecutionStep>> {
        let mut steps = Vec::new();

        for (index, step) in generated_steps.into_iter().enumerate() {
            let role = match step.assigned_role.as_str() {
                "planner" => AgentRole::Planner,
                "worker" => AgentRole::Worker,
                "critic" => AgentRole::Critic,
                "researcher" => AgentRole::Researcher,
                "builder" => AgentRole::Builder,
                "reporter" => AgentRole::Reporter,
                other => {
                    return Err(Agent1Error::Config(format!(
                        "unknown agent role in plan: {other}"
                    )));
                }
            };

            let dependencies: Vec<String> = step
                .dependencies
                .iter()
                .filter_map(|d| {
                    if let Ok(idx) = d.parse::<usize>() {
                        if idx < index {
                            return Some(format!("step_{}", new_id("dep").split('_').last().unwrap_or("")));
                        }
                    }
                    None
                })
                .collect();

            let mut execution_step = ExecutionStep::new(
                plan_id.clone(),
                step.description,
                index,
                dependencies,
            );
            execution_step.assigned_role = Some(role);

            steps.push(execution_step);
        }

        if steps.len() > self.config.max_plan_depth {
            return Err(Agent1Error::Config(format!(
                "plan has {} steps, exceeds maximum of {}",
                steps.len(),
                self.config.max_plan_depth
            )));
        }

        Ok(steps)
    }

    pub fn validate_dependencies(&self, steps: &[ExecutionStep]) -> Result<()> {
        let step_ids: HashSet<&str> = steps.iter().map(|s| s.id.as_str()).collect();
        let mut visited = HashSet::new();

        for step in steps {
            for dep in &step.dependencies {
                if !step_ids.contains(dep.as_str()) {
                    return Err(Agent1Error::Config(format!(
                        "step {} references unknown dependency {dep}",
                        step.id
                    )));
                }
            }

            if !visited.insert(step.id.as_str()) {
                return Err(Agent1Error::Config(format!(
                    "circular dependency detected involving step {}",
                    step.id
                )));
            }
        }

        Ok(())
    }

    pub fn get_ready_steps<'a>(
        &self,
        steps: &'a [ExecutionStep],
        completed: &HashSet<String>,
    ) -> Vec<&'a ExecutionStep> {
        steps
            .iter()
            .filter(|step| {
                step.status == agent1_core::StepStatus::Pending
                    && step.dependencies.iter().all(|dep| completed.contains(dep))
            })
            .collect()
    }
}

impl Default for GoalDecomposer {
    fn default() -> Self {
        Self::new(OrchestratorConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn parse_simple_plan() {
        let decomposer = GoalDecomposer::default();
        let json = r#"{
            "summary": "Build a simple web app",
            "steps": [
                {"description": "Create project structure", "dependencies": [], "assigned_role": "builder"},
                {"description": "Write main code", "dependencies": [0], "assigned_role": "builder"},
                {"description": "Review code", "dependencies": [1], "assigned_role": "critic"}
            ]
        }"#;

        let parsed = decomposer.parse_plan_response(json).expect("should parse");
        assert_eq!(parsed.steps.len(), 3);
        assert_eq!(parsed.steps[0].assigned_role, "builder");
        assert!(parsed.steps[0].dependencies.is_empty());
        assert_eq!(parsed.steps[2].dependencies, vec!["1"]);
    }

    #[tokio::test]
    async fn parse_plan_with_markdown() {
        let decomposer = GoalDecomposer::default();
        let json = r#"```json
        {
            "summary": "Test",
            "steps": [{"description": "Do thing", "dependencies": [], "assigned_role": "worker"}]
        }
        ```"#;

        let parsed = decomposer.parse_plan_response(json).expect("should parse");
        assert_eq!(parsed.steps.len(), 1);
    }
}