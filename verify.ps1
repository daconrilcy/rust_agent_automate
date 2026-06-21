[CmdletBinding()]
param(
    [string]$TargetDir
)

$ErrorActionPreference = 'Stop'

$RepoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
if ([string]::IsNullOrWhiteSpace($TargetDir)) {
    $TargetDir = Join-Path $RepoRoot '.target-verify'
} elseif (-not [System.IO.Path]::IsPathRooted($TargetDir)) {
    $TargetDir = Join-Path $RepoRoot $TargetDir
}

function Invoke-Cargo {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$Arguments
    )

    Write-Host "> cargo $($Arguments -join ' ')"
    & cargo @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "cargo $($Arguments -join ' ') failed with exit code $LASTEXITCODE"
    }
}

$previousCargoTargetDir = $env:CARGO_TARGET_DIR
$hadCargoTargetDir = Test-Path Env:CARGO_TARGET_DIR
$pushedLocation = $false

try {
    Push-Location $RepoRoot
    $pushedLocation = $true
    $env:CARGO_TARGET_DIR = $TargetDir
    Write-Host "CARGO_TARGET_DIR=$env:CARGO_TARGET_DIR"

    Invoke-Cargo @('fmt', '--check')
    Invoke-Cargo @('clippy', '--workspace', '--all-targets', '--', '-D', 'warnings')
    Invoke-Cargo @('test', '--test', 'service_parser')
    Invoke-Cargo @('test', '--test', 'workflow_chain')
    Invoke-Cargo @('test')
} finally {
    if ($pushedLocation) {
        Pop-Location
    }
    if ($hadCargoTargetDir) {
        $env:CARGO_TARGET_DIR = $previousCargoTargetDir
    } else {
        Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
    }
}
