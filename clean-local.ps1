[CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Medium')]
param(
    [switch]$IncludeArtifacts,
    [switch]$Quiet
)

$ErrorActionPreference = 'Stop'

$RepoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRootItem = Get-Item -LiteralPath $RepoRoot

function Get-RepoRelativePath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $root = $RepoRootItem.FullName.TrimEnd([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar)
    if ($Path.StartsWith($root, [StringComparison]::OrdinalIgnoreCase)) {
        $relative = $Path.Substring($root.Length).TrimStart([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar)
        if ($relative) {
            return ".\$relative"
        }
    }

    return $Path
}

function Test-ContainsTrackedGitFile {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
        return $false
    }

    $relativePath = Get-RepoRelativePath -Path $Path
    $gitPath = $relativePath -replace '^\.\\', ''
    $gitPath = $gitPath -replace '\\', '/'

    $tracked = & git -C $RepoRootItem.FullName ls-files -- $gitPath 2>$null
    return [bool]$tracked
}

$artifactDirs = @(
    '.agents',
    '.audit',
    '.fix-loop',
    '.plan'
)

$transientPatterns = @(
    '.audit_tmp_target',
    '.cargo-target-loop-*',
    '.cargo-target-verify*',
    '.target*',
    '.target-verify*',
    'target',
    'target-audit-verify',
    'target-verify*',
    'target*'
)

$transientFilePatterns = @(
    '*.tmp',
    '*.temp',
    '*.log',
    '*.pid',
    '*.out',
    '*.rs.bk'
)

$directoryPatterns = @()
if ($IncludeArtifacts) {
    $directoryPatterns += $artifactDirs
}
$directoryPatterns += $transientPatterns

$directoryTargets = foreach ($pattern in $directoryPatterns) {
    Get-ChildItem -LiteralPath $RepoRootItem.FullName -Force -Directory -Filter $pattern -ErrorAction SilentlyContinue
}

$directoryTargets = $directoryTargets |
    Where-Object {
        $_.FullName.StartsWith($RepoRootItem.FullName, [StringComparison]::OrdinalIgnoreCase) -and
        $_.FullName -ne $RepoRootItem.FullName -and
        $_.Name -ne '.git' -and
        -not (Test-ContainsTrackedGitFile -Path $_.FullName)
    } |
    Sort-Object FullName -Unique

$fileTargets = foreach ($pattern in $transientFilePatterns) {
    Get-ChildItem -LiteralPath $RepoRootItem.FullName -Force -File -Filter $pattern -ErrorAction SilentlyContinue
}

$fileTargets = $fileTargets |
    Where-Object {
        $_.FullName.StartsWith($RepoRootItem.FullName, [StringComparison]::OrdinalIgnoreCase) -and
        $_.Name -notin @('.env', '.env.example', '.gitignore') -and
        -not (Test-ContainsTrackedGitFile -Path $_.FullName)
    } |
    Sort-Object FullName -Unique

if ((-not $directoryTargets) -and (-not $fileTargets)) {
    if (-not $Quiet) {
        Write-Host 'No local generated state found.'
    }
    return
}

foreach ($target in $directoryTargets) {
    $relativePath = Get-RepoRelativePath -Path $target.FullName
    if ($PSCmdlet.ShouldProcess($relativePath, 'Remove local generated directory')) {
        Remove-Item -LiteralPath $target.FullName -Recurse -Force
        if (-not $Quiet) {
            Write-Host "Removed $relativePath"
        }
    }
}

foreach ($target in $fileTargets) {
    $relativePath = Get-RepoRelativePath -Path $target.FullName
    if ($PSCmdlet.ShouldProcess($relativePath, 'Remove local temporary file')) {
        Remove-Item -LiteralPath $target.FullName -Force
        if (-not $Quiet) {
            Write-Host "Removed $relativePath"
        }
    }
}
