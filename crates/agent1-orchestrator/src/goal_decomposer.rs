use crate::types::{ExecutionPlan, ExecutionStep, OrchestratorConfig, PlanView};
use agent1_core::{
    new_id, now, Agent1Error, AgentRole, ChatMessage, ChatRequest, PlanId, PlanStatus, Result,
};
use agent1_models::provider_for;
use serde::{Deserialize, Deserializer};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Deserialize)]
struct PlanStep {
    description: String,
    #[serde(default, deserialize_with = "deserialize_dependency_indices")]
    dependencies: Vec<String>,
    assigned_role: String,
    #[serde(default)]
    sub_steps: Vec<PlanStep>,
}

#[derive(Debug, Deserialize)]
struct GeneratedPlan {
    steps: Vec<PlanStep>,
}

fn deserialize_dependency_indices<'de, D>(
    deserializer: D,
) -> std::result::Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let values = Vec::<serde_json::Value>::deserialize(deserializer)?;
    values
        .into_iter()
        .map(|value| match value {
            serde_json::Value::String(text) => Ok(text),
            serde_json::Value::Number(number) => Ok(number.to_string()),
            other => Err(serde::de::Error::custom(format!(
                "dependency index must be string or number, got {other}"
            ))),
        })
        .collect()
}

pub struct GoalDecomposer {
    config: OrchestratorConfig,
}

impl GoalDecomposer {
    pub fn new(config: OrchestratorConfig) -> Self {
        Self { config }
    }

    pub async fn decompose(&self, goal: &str, orchestration_id: &str) -> Result<PlanView> {
        let plan = ExecutionPlan::new(
            orchestration_id.to_string(),
            format!("Plan for: {}", goal),
            goal.to_string(),
        );

        let prompt = self.build_decomposition_prompt(goal);
        let generated = match self.call_planner(&prompt).await {
            Ok(plan_json) => self
                .parse_plan_response(&plan_json)
                .unwrap_or_else(|_| self.fallback_plan(goal)),
            Err(_) => self.fallback_plan(goal),
        };

        let (steps, sub_steps) = self.build_steps(&plan.id, generated.steps)?;

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
            sub_steps,
        })
    }

    pub async fn decompose_with_context(
        &self,
        goal: &str,
        orchestration_id: &str,
        context: &[String],
    ) -> Result<PlanView> {
        let plan = ExecutionPlan::new(
            orchestration_id.to_string(),
            format!("Plan for: {}", goal),
            goal.to_string(),
        );

        let prompt = self.build_contextual_decomposition_prompt(goal, context);
        let generated = match self.call_planner(&prompt).await {
            Ok(plan_json) => self
                .parse_plan_response(&plan_json)
                .unwrap_or_else(|_| self.fallback_plan(goal)),
            Err(_) => self.fallback_plan(goal),
        };

        let (steps, sub_steps) = self.build_steps(&plan.id, generated.steps)?;

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
            sub_steps,
        })
    }

    fn build_decomposition_prompt(&self, goal: &str) -> String {
        self.build_decomposition_prompt_with_context(goal, &[])
    }

    fn build_contextual_decomposition_prompt(&self, goal: &str, context: &[String]) -> String {
        self.build_decomposition_prompt_with_context(goal, context)
    }

    fn build_decomposition_prompt_with_context(&self, goal: &str, context: &[String]) -> String {
        let context_section = if context.is_empty() {
            String::new()
        } else {
            format!(
                "\n\nRelevant Context from Past Sessions:\n{}\n\nUse this context to inform your planning when relevant.",
                context.iter().enumerate().map(|(i, c)| format!("{}. {}", i + 1, c)).collect::<Vec<_>>().join("\n")
            )
        };

        format!(
            r#"You are a Planner agent helping break down a complex objective into execution steps.

Objective: {}

{}

Generate a detailed execution plan as JSON with this exact format:
{{
  "summary": "Brief summary of the plan approach",
  "steps": [
    {{
      "description": "Clear description of what this step does",
      "dependencies": ["list of step indices this depends on, e.g. [0] for first step"],
      "assigned_role": "planner|worker|critic|researcher|builder|reporter",
      "sub_steps": [
        {{
          "description": "Sub-step description",
          "dependencies": ["indices relative to parent step's sub-steps"],
          "assigned_role": "worker|builder|researcher"
        }}
      ]
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
10. Include an explicit validation or verification step for code, configuration, or workflow changes
11. Keep descriptions concise but actionable
12. When context mentions specific tools, patterns, or approaches, consider using them
13. For complex steps (e.g., "build a web server", "create database schema"), add 2-5 sub_steps
14. Sub-steps are for breaking down complex tasks into smaller, verifiable parts
15. Sub-steps run as part of their parent step and share the parent's context

Return ONLY valid JSON, no markdown or explanation."#,
            goal, context_section
        )
    }

    async fn call_planner(&self, prompt: &str) -> Result<String> {
        let model = self.config.model_routing.planner.clone();
        let provider = provider_for(&model)?;

        let request = ChatRequest {
            model,
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
        };

        let response = provider.chat(request).await?;
        Ok(response.content)
    }

    fn fallback_plan(&self, goal: &str) -> GeneratedPlan {
        GeneratedPlan {
            steps: vec![PlanStep {
                description: goal.to_string(),
                dependencies: Vec::new(),
                assigned_role: "worker".to_string(),
                sub_steps: Vec::new(),
            }],
        }
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
            Agent1Error::InvalidModelResponse(format!(
                "failed to parse plan JSON: {err}, content: {cleaned}"
            ))
        })
    }

    fn build_steps(
        &self,
        plan_id: &PlanId,
        generated_steps: Vec<PlanStep>,
    ) -> Result<(Vec<ExecutionStep>, HashMap<PlanId, Vec<ExecutionStep>>)> {
        let mut steps = Vec::new();
        let mut pending_dependencies = Vec::new();
        let mut sub_steps_map: HashMap<PlanId, Vec<ExecutionStep>> = HashMap::new();

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

            let mut execution_step =
                ExecutionStep::new(plan_id.clone(), step.description, index, Vec::new());
            execution_step.assigned_role = Some(role);

            if !step.sub_steps.is_empty() {
                let sub_plan_id = new_id("plan");
                let sub_steps = self.build_sub_steps(&sub_plan_id, step.sub_steps, 0)?;
                let sub_steps_count = sub_steps.len();
                execution_step.sub_plan_id = Some(sub_plan_id.clone());
                sub_steps_map.insert(sub_plan_id.clone(), sub_steps);
                tracing::debug!(
                    "Created sub-plan {} with {} steps for step {}",
                    sub_plan_id,
                    sub_steps_count,
                    execution_step.id
                );
            }

            steps.push(execution_step);
            pending_dependencies.push(step.dependencies);
        }

        let step_ids = steps.iter().map(|step| step.id.clone()).collect::<Vec<_>>();
        for (index, dependencies) in pending_dependencies.into_iter().enumerate() {
            let resolved = dependencies
                .into_iter()
                .map(|dependency| {
                    let dependency_index = dependency.parse::<usize>().map_err(|_| {
                        Agent1Error::Config(format!(
                            "dependency `{dependency}` is not a valid step index"
                        ))
                    })?;
                    if dependency_index >= index {
                        return Err(Agent1Error::Config(format!(
                            "step {index} depends on non-prior step {dependency_index}"
                        )));
                    }
                    step_ids.get(dependency_index).cloned().ok_or_else(|| {
                        Agent1Error::Config(format!(
                            "dependency index {dependency_index} is outside the plan"
                        ))
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            steps[index].dependencies = resolved;
        }

        if steps.len() > self.config.max_plan_depth {
            return Err(Agent1Error::Config(format!(
                "plan has {} steps, exceeds maximum of {}",
                steps.len(),
                self.config.max_plan_depth
            )));
        }

        Ok((steps, sub_steps_map))
    }

    fn build_sub_steps(
        &self,
        plan_id: &PlanId,
        generated_steps: Vec<PlanStep>,
        base_order: usize,
    ) -> Result<Vec<ExecutionStep>> {
        let mut steps = Vec::new();
        let mut pending_dependencies = Vec::new();

        for (index, step) in generated_steps.into_iter().enumerate() {
            let order = base_order * 100 + index;
            let role = match step.assigned_role.as_str() {
                "worker" => AgentRole::Worker,
                "builder" => AgentRole::Builder,
                "researcher" => AgentRole::Researcher,
                "critic" => AgentRole::Critic,
                "planner" => AgentRole::Planner,
                "reporter" => AgentRole::Reporter,
                other => {
                    tracing::warn!("Unknown role in sub-steps: {}, defaulting to worker", other);
                    AgentRole::Worker
                }
            };

            let mut execution_step =
                ExecutionStep::new(plan_id.clone(), step.description, order, Vec::new());
            execution_step.assigned_role = Some(role);

            if !step.sub_steps.is_empty() {
                tracing::warn!("Nested sub-steps not supported, ignoring");
            }

            steps.push(execution_step);
            pending_dependencies.push(step.dependencies);
        }

        let step_ids = steps.iter().map(|step| step.id.clone()).collect::<Vec<_>>();
        for (index, dependencies) in pending_dependencies.into_iter().enumerate() {
            let resolved: Vec<_> = dependencies
                .into_iter()
                .filter_map(|dependency| {
                    let dependency_index = match dependency.parse::<usize>() {
                        Ok(i) => i,
                        Err(_) => return None,
                    };
                    if dependency_index >= index {
                        return None;
                    }
                    step_ids.get(dependency_index).cloned()
                })
                .collect();
            steps[index].dependencies = resolved;
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
