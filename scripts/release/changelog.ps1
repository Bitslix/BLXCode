# CHANGELOG [Unreleased] -> [version] - dot-source only.

function Invoke-ReleaseChangelogFinalize {
    param([Parameter(Mandatory = $true)][string]$Version)

    $changelog = Get-ReleaseChangelogPath
    $text = [System.IO.File]::ReadAllText($changelog, [System.Text.UTF8Encoding]::new($false, $true))
    $unreleasedHeader = "## [Unreleased]"
    $newUnreleased = @"
## [Unreleased]

### Added

### Changed

### Fixed

### Removed

"@

    $idx = $text.IndexOf($unreleasedHeader, [StringComparison]::Ordinal)
    if ($idx -lt 0) {
        Stop-Release "${changelog}: missing '$unreleasedHeader'"
    }

    $after = $idx + $unreleasedHeader.Length
    $rest = $text.Substring($after)
    $nextMatch = [regex]::Match($rest, "`n## \[")
    if ($nextMatch.Success) {
        $body = $rest.Substring(0, $nextMatch.Index)
        $tail = $rest.Substring($nextMatch.Index)
    } else {
        $body = $rest
        $tail = ""
    }

    if (-not $body.Trim()) {
        Write-ReleaseWarn "[Unreleased] section is empty"
    }

    $releaseDate = (Get-Date).ToString("yyyy-MM-dd")
    $versioned = "## [$Version] - $releaseDate"
    $newText = $text.Substring(0, $idx) + $newUnreleased + [Environment]::NewLine + $versioned + $body + $tail

    if ($script:RELEASE_DRY_RUN -eq 1) {
        Write-Host "--- $changelog (dry-run preview, first 40 lines of result) ---"
        $lines = $newText -split "`r?`n"
        $limit = [Math]::Min(40, $lines.Count)
        for ($i = 0; $i -lt $limit; $i++) {
            Write-Host $lines[$i]
        }
        if ($lines.Count -gt 40) {
            Write-Host "..."
        }
        return
    }

    [System.IO.File]::WriteAllText($changelog, $newText, [System.Text.UTF8Encoding]::new($false))
    Write-ReleaseInfo "updated $changelog"
}
