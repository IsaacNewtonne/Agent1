param(
    [string]$Workspace = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path,
    [string]$Db = ".agent1/agent1.db",
    [string]$Notes = ".agent1/external-intel-notes.md",
    [int]$MaxRuns = 1,
    [switch]$AutoApprove
)

$ErrorActionPreference = "Stop"

Set-Location $Workspace

$digestDir = Join-Path $Workspace "Agent1_Project_Docs/external-intelligence"
New-Item -ItemType Directory -Force -Path $digestDir | Out-Null

$task = @"
Run the Agent1 External Intelligence Loop.

Read Agent1_Project_Docs/SELF_IMPROVE.md and Agent1_Project_Docs/EXTERNAL_INTELLIGENCE_LOOP.md first.

Research current open-source agent, MCP, workflow orchestration, eval, local-first AI, and desktop automation architectures. Prefer architecture-bearing repositories, official docs, benchmark updates, and implementation writeups. De-prioritize thin wrappers, generic chat shells, UI-only demos, and trend claims without inspectable architecture.

Write a dated digest under Agent1_Project_Docs/external-intelligence/ using today's date. Include:
- sources reviewed with links
- architectural patterns worth noticing
- applicability to Agent1
- risks and unknowns
- ranked one-change experiments
- eval or verification needed before adoption

Update the persistent notes file with what changed and the next research focus.

Do not modify production code, prompts, approval policy, security policy, dependencies, or generated lockfiles. Do not create commits. This loop may only write the digest, update notes, and propose bounded follow-up tasks.
"@

$argsList = @(
    "run", "--bin", "agent1", "--", "loop",
    "--task", $task,
    "--workspace", $Workspace,
    "--db", $Db,
    "--max-runs", "$MaxRuns",
    "--completion-signal", "AGENT1_EXTERNAL_INTEL_COMPLETE",
    "--completion-threshold", "1",
    "--notes", $Notes
)

if ($AutoApprove) {
    $argsList += "--auto-approve"
}

cargo @argsList
