param(
    [string]$Workspace = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path,
    [string]$TaskName = "Agent1 External Intelligence",
    [string]$At = "03:00",
    [ValidateSet("Daily", "Weekly")]
    [string]$Schedule = "Daily"
)

$ErrorActionPreference = "Stop"

$runner = Join-Path $Workspace "scripts/agent1-external-intel.ps1"
if (-not (Test-Path $runner)) {
    throw "Missing runner script: $runner"
}

$action = New-ScheduledTaskAction `
    -Execute "powershell.exe" `
    -Argument "-NoProfile -ExecutionPolicy Bypass -File `"$runner`" -Workspace `"$Workspace`""

if ($Schedule -eq "Weekly") {
    $trigger = New-ScheduledTaskTrigger -Weekly -DaysOfWeek Monday -At $At
} else {
    $trigger = New-ScheduledTaskTrigger -Daily -At $At
}

$settings = New-ScheduledTaskSettingsSet `
    -ExecutionTimeLimit (New-TimeSpan -Hours 2) `
    -MultipleInstances IgnoreNew `
    -StartWhenAvailable

Register-ScheduledTask `
    -TaskName $TaskName `
    -Action $action `
    -Trigger $trigger `
    -Settings $settings `
    -Description "Runs Agent1's proposal-only external intelligence loop." `
    -Force | Out-Null

Write-Host "Registered scheduled task '$TaskName' ($Schedule at $At)."
Write-Host "Runner: $runner"
