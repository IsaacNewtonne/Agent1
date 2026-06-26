param(
    [string]$Workspace = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path,
    [string]$Db = ".agent1/agent1.db",
    [int]$MaxRuns = 1,
    [switch]$AutoApprove
)

$ErrorActionPreference = "Stop"

Set-Location $Workspace

$digestDir = Join-Path $Workspace "Agent1_Project_Docs/external-intelligence"
New-Item -ItemType Directory -Force -Path $digestDir | Out-Null

$argsList = @(
    "run", "--bin", "agent1", "--", "loops", "run", "external-intelligence",
    "--workspace", $Workspace,
    "--db", $Db,
    "--max-runs", "$MaxRuns"
)

if ($AutoApprove) {
    $argsList += "--auto-approve"
}

cargo @argsList
