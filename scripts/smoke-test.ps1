param(
    [string]$ApiBase = "http://127.0.0.1:17371",
    [string]$DbPath = "$env:TEMP\agent1-smoke-test.db",
    [int]$Timeout = 30,
    [switch]$AllowExternalDbPath
)

$ErrorAction = "Stop"

function Get-Status {
    try {
        $r = Invoke-RestMethod "$ApiBase/api/health" -TimeoutSec 5
        return $r.ok
    } catch {
        return $false
    }
}

Write-Host "Agent1 Smoke Test" -ForegroundColor Cyan
Write-Host "==================" -ForegroundColor Cyan

$serverProc = $null
$resolvedTemp = [System.IO.Path]::GetFullPath($env:TEMP)
$resolvedDbPath = [System.IO.Path]::GetFullPath($DbPath)
if (-not $AllowExternalDbPath -and -not $resolvedDbPath.StartsWith($resolvedTemp, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "DbPath must be under TEMP unless -AllowExternalDbPath is set: $resolvedDbPath"
}

try {
    Write-Host "[1/5] Starting server..." -ForegroundColor Yellow
    
    if (Test-Path $DbPath) { Remove-Item $DbPath -Force }
    
    $serverProc = Start-Process -FilePath "cargo" -ArgumentList "run","-q","--bin","agent1","--","server","--db",$DbPath -PassThru -WindowStyle Hidden
    
    $waited = 0
    while (-not (Get-Status) -and $waited -lt $Timeout) {
        Start-Sleep 1
        $waited++
        Write-Host "." -NoNewline
    }
    
    if (-not (Get-Status)) {
        throw "Server did not start within $Timeout seconds"
    }
    
    Write-Host " OK" -ForegroundColor Green

    Write-Host "[2/5] Testing health endpoint..." -ForegroundColor Yellow
    $health = Invoke-RestMethod "$ApiBase/api/health"
    if ($health.ok -ne $true) { throw "Health check failed" }
    Write-Host " OK" -ForegroundColor Green

    Write-Host "[3/5] Creating test agent..." -ForegroundColor Yellow
    $agentBody = @{
        id = "smoke_test_agent"
        name = "Smoke Test"
        system_prompt = "You are a test agent."
        tools = @()
        model = @{
            provider = "mock"
            model = "final"
            context_window = 8192
            temperature = 0.2
        }
        max_iterations = 1
    } | ConvertTo-Json
    
    $null = Invoke-RestMethod "$ApiBase/api/agents" -Method POST -Body $agentBody -ContentType "application/json"
    Write-Host " OK" -ForegroundColor Green

    Write-Host "[4/5] Running test task..." -ForegroundColor Yellow
    $runBody = @{
        agent_id = "smoke_test_agent"
        input = "echo hello"
        workspace = "."
    } | ConvertTo-Json
    
    $result = Invoke-RestMethod "$ApiBase/api/sessions/run" -Method POST -Body $runBody -ContentType "application/json"
    
    if (-not $result.session_id) { throw "No session ID returned" }
    Write-Host " OK" -ForegroundColor Green

    Write-Host "[5/5] Checking for panics..." -ForegroundColor Yellow
    if ($serverProc.HasExited) { throw "Server crashed with exit code $($serverProc.ExitCode)" }
    Write-Host " OK" -ForegroundColor Green

    Write-Host ""
    Write-Host "ALL TESTS PASSED" -ForegroundColor Green

} catch {
    Write-Host ""
    Write-Host "TEST FAILED: $_" -ForegroundColor Red
    exit 1
    
} finally {
    if ($serverProc -and -not $serverProc.HasExited) {
        Stop-Process $serverProc.Id -Force -ErrorAction SilentlyContinue
    }
    if (Test-Path $DbPath) { Remove-Item $DbPath -Force -ErrorAction SilentlyContinue }
}
