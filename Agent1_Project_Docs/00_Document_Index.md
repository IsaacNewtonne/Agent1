# Agent1 Document Index

## Product

**Agent1** is an open-source, self-hosted, Rust-first agent platform for building, running, connecting, observing, and governing local AI agents. It is designed for local models, local tools, local memory, MCP-compatible tool/context integration, and A2A-style agent-to-agent collaboration.

## Selected Documentation Set

These files are required for an AI or engineering team to build Agent1 correctly:

- `00_Document_Index.md`
- `00_Repository_Readme.md`
- `00_Glossary_And_Definitions.md`
- `00_Decision_Log_ADR.md`
- `01_Product_Requirements_PRD.md`
- `01_Vision_Goals_NonGoals.md`
- `01_User_Personas_And_Jobs_To_Be_Done.md`
- `01_Use_Cases.md`
- `01_User_Stories_And_Acceptance_Criteria.md`
- `01_Scope_MVP_And_Roadmap.md`
- `01_NonFunctional_Requirements_NFR.md`
- `01_Risk_Register_And_Assumptions.md`
- `01_Known_Unknowns_And_Open_Questions.md`
- `02_Project_Plan_Timeline_And_Milestones.md`
- `02_Backlog_Prioritization.md`
- `02_Release_Plan_And_Versioning.md`
- `03_System_Architecture_Overview.md`
- `03_Tech_Stack_Lock.md`
- `03_Async_Runtime_And_Concurrency.md`
- `03_Component_Design_Modules_And_Interfaces.md`
- `03_Functions_Objects_And_Attributes.md`
- `03_Flow_Diagrams_Mermaid.md`
- `03_API_Contracts_And_Schemas.md`
- `03_External_Events_And_WebSocket_Schema.md`
- `03_Error_Handling_Retries_Timeouts.md`
- `04_Data_Plan.md`
- `04_Database_Schema_And_Migrations.md`
- `04_Data_Governance_Retention_Backups.md`
- `05_UI_Plan.md`
- `05_UX_Flows_And_Wireframes.md`
- `05_Design_System_And_Accessibility.md`
- `06_Client_Integration_Spec.md`
- `06_Configuration_And_Feature_Flags.md`
- `07_Security_Privacy_Compliance_Plan.md`
- `07_Threat_Model.md`
- `07_Secrets_And_Key_Management.md`
- `08_QA_Test_Strategy.md`
- `08_Test_Cases_And_Checklists.md`
- `09_Observability_Spec.md`
- `09_Runbook_And_Operations.md`
- `09_Incident_Response.md`
- `10_Deployment_Architecture.md`
- `10_CI_CD_Pipeline.md`

## Intentionally Excluded From Initial Build Pack

The following files from the original list are not required for the first build pack:

- `00_Change_Log.md` — useful after implementation starts, but empty for a new project.
- `01_Success_Metrics_Analytics_Plan.md` — product analytics are intentionally minimal because Agent1 is local-first and privacy-first. Basic success criteria are included in the PRD and roadmap.
- `06_Domain_Constraints_And_Source_Allowlist.md` — not relevant as a standalone document because Agent1 has no required hosted API domains. Network and tool allowlists are covered in Security, Config, and Tool Permissions.
- `07_Data_Privacy_DPIA_Notes.md` — full DPIA is not needed for an offline-first open-source MVP. Privacy obligations are covered in Security and Data Governance.

## Build Order

1. Read `00_Repository_Readme.md`
2. Read `01_Product_Requirements_PRD.md`
3. Read `01_Scope_MVP_And_Roadmap.md`
4. Read `03_System_Architecture_Overview.md`
5. Read `03_Tech_Stack_Lock.md`
6. Read all `03_*` technical specs
7. Read `04_*` data specs
8. Read `07_*` security specs
9. Read `08_*` QA specs
10. Read `10_*` deployment specs

## Core Rule

Agent1 must be:

> Local by default, permissioned by default, observable by default, and replaceable by design.
