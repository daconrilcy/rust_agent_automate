[CmdletBinding()]
param(
    [string]$InstallRoot,
    [string]$CommandName = 'rust_agent',
    [string]$Package = 'app',
    [string]$TargetDir,
    [switch]$NoPathUpdate
)

$ErrorActionPreference = 'Stop'

$RepoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
if ([string]::IsNullOrWhiteSpace($InstallRoot)) {
    $InstallRoot = Join-Path $env:LOCALAPPDATA 'Programs'
} elseif (-not [System.IO.Path]::IsPathRooted($InstallRoot)) {
    $InstallRoot = Join-Path $RepoRoot $InstallRoot
}

if ([string]::IsNullOrWhiteSpace($TargetDir)) {
    $TargetDir = Join-Path $RepoRoot '.target-install'
} elseif (-not [System.IO.Path]::IsPathRooted($TargetDir)) {
    $TargetDir = Join-Path $RepoRoot $TargetDir
}

if ([string]::IsNullOrWhiteSpace($CommandName)) {
    throw 'CommandName cannot be empty.'
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

function Add-UserPathEntry {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $normalizedPath = [System.IO.Path]::GetFullPath($Path).TrimEnd(
        [System.IO.Path]::DirectorySeparatorChar,
        [System.IO.Path]::AltDirectorySeparatorChar
    )
    $currentUserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $userEntries = @()
    if (-not [string]::IsNullOrWhiteSpace($currentUserPath)) {
        $userEntries = $currentUserPath -split ';' | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
    }

    $alreadyInUserPath = $userEntries | Where-Object {
        $entry = [System.IO.Path]::GetFullPath($_).TrimEnd(
            [System.IO.Path]::DirectorySeparatorChar,
            [System.IO.Path]::AltDirectorySeparatorChar
        )
        [string]::Equals($entry, $normalizedPath, [StringComparison]::OrdinalIgnoreCase)
    }

    if ($alreadyInUserPath) {
        Write-Host "PATH utilisateur deja configure: $normalizedPath"
    } else {
        $updatedUserPath = (($userEntries + $normalizedPath) -join ';')
        [Environment]::SetEnvironmentVariable('Path', $updatedUserPath, 'User')
        Write-Host "PATH utilisateur mis a jour: $normalizedPath"
    }

    $processEntries = @()
    if (-not [string]::IsNullOrWhiteSpace($env:Path)) {
        $processEntries = $env:Path -split ';' | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
    }

    $alreadyInProcessPath = $processEntries | Where-Object {
        $entry = [System.IO.Path]::GetFullPath($_).TrimEnd(
            [System.IO.Path]::DirectorySeparatorChar,
            [System.IO.Path]::AltDirectorySeparatorChar
        )
        [string]::Equals($entry, $normalizedPath, [StringComparison]::OrdinalIgnoreCase)
    }

    if (-not $alreadyInProcessPath) {
        $env:Path = (($processEntries + $normalizedPath) -join ';')
        Write-Host "PATH du terminal courant mis a jour: $normalizedPath"
    }

    if (-not $alreadyInUserPath) {
        Write-Host 'Les nouveaux terminaux heriteront du PATH mis a jour.'
    }
}

$installDir = Join-Path $InstallRoot $CommandName
$installedExe = Join-Path $installDir "$CommandName.exe"
$previousCargoTargetDir = $env:CARGO_TARGET_DIR
$hadCargoTargetDir = Test-Path Env:CARGO_TARGET_DIR
$pushedLocation = $false

try {
    Push-Location $RepoRoot
    $pushedLocation = $true
    $env:CARGO_TARGET_DIR = $TargetDir
    Write-Host "CARGO_TARGET_DIR=$env:CARGO_TARGET_DIR"

    Invoke-Cargo @('build', '--release', '-p', $Package)

    $builtExe = Join-Path $TargetDir "release\$Package.exe"
    if (-not (Test-Path -LiteralPath $builtExe)) {
        throw "Compiled executable not found: $builtExe"
    }

    New-Item -ItemType Directory -Force -Path $installDir | Out-Null
    Copy-Item -LiteralPath $builtExe -Destination $installedExe -Force

    $metadata = [ordered]@{
        command = $CommandName
        package = $Package
        source = $RepoRoot
        installed_exe = $installedExe
        installed_at = (Get-Date).ToString('o')
    }
    $metadata | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $installDir 'install.json') -Encoding UTF8

    if (-not $NoPathUpdate) {
        Add-UserPathEntry -Path $installDir
    }

    Write-Host "Installe: $installedExe"
    if ($NoPathUpdate) {
        Write-Host "Commande installee hors mise a jour PATH: $installedExe"
    } else {
        Write-Host "Commande disponible: $CommandName"
    }
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
