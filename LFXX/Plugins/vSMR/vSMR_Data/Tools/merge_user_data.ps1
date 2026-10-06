# Dot-sourced only after the installer has validated the package hashes.
# Baselines are previous upstream defaults, never snapshots of edited user data.
function Initialize-UserDataMerge([string]$InstalledData, [string]$StagedData, [string]$Release) {
    if (-not ('Vsmr.Updates.JsonThreeWayMerge' -as [type])) {
        Add-Type -Path (Join-Path $PSScriptRoot 'JsonThreeWayMerge.cs')
    }
    $script:mergeInstalledData = $InstalledData
    $script:mergeStageData = $StagedData
    $script:mergeOldHashes = @{}
    $script:mergeNewHashes = @{}
    $script:mergeReport = [ordered]@{ schema_version = 1; release = $Release; files = @() }
    $index = Join-Path $InstalledData 'UpdateBaselines/BASELINE-HASHES.json'
    if (Test-Path -LiteralPath $index -PathType Leaf) {
        try {
            if ((Get-Item -LiteralPath $index).Length -gt 1MB) { throw 'Oversized baseline index.' }
            $saved = Get-Content -LiteralPath $index -Raw | ConvertFrom-Json
            if ($saved.schema_version -ne 1) { throw 'Unsupported baseline index.' }
            foreach ($entry in $saved.files.PSObject.Properties) {
                $name = [string]$entry.Name
                if ($name -notmatch '^(vSMR_Profiles\.json|AVISO/[A-Za-z0-9]{4}(?:_[A-Za-z0-9][A-Za-z0-9_-]{0,47})?\.geojson)$' -or
                    $entry.Value -notmatch '^[0-9a-f]{64}$') { throw 'Invalid baseline entry.' }
                $script:mergeOldHashes[$name] = [string]$entry.Value
            }
        } catch { $script:mergeOldHashes = @{} }
    }
    foreach ($name in @($script:mergeOldHashes.Keys)) {
        $baseline = Get-UserDataBaseline $name
        if ($baseline) { Save-UserDataBaseline $name $baseline }
    }
}

function Get-UserDataBaseline([string]$RelativePath) {
    if (-not $script:mergeOldHashes.ContainsKey($RelativePath)) { return $null }
    $path = Join-Path $script:mergeInstalledData ('UpdateBaselines/' + $RelativePath)
    if (Test-Path -LiteralPath $path -PathType Leaf) {
        if ((Get-FileSha256 $path) -eq $script:mergeOldHashes[$RelativePath]) { return $path }
    }
    return $null
}

function Save-UserDataBaseline([string]$RelativePath, [string]$Source) {
    $target = Join-Path $script:mergeStageData ('UpdateBaselines/' + $RelativePath)
    Assert-ChildPath $target (Join-Path $script:mergeStageData 'UpdateBaselines')
    [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($target)) | Out-Null
    Copy-Item -LiteralPath $Source -Destination $target -Force
    $script:mergeNewHashes[$RelativePath] = Get-FileSha256 $target
}

function Save-IncomingUserData([string]$RelativePath, [string]$Incoming) {
    $target = Join-Path $script:mergeStageData ('Data_Updates/' + $script:mergeReport.release + '/' + $RelativePath)
    Assert-ChildPath $target (Join-Path $script:mergeStageData 'Data_Updates')
    [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($target)) | Out-Null
    Copy-Item -LiteralPath $Incoming -Destination $target -Force
}

function Merge-UserDataFile([string]$RelativePath, [string]$Incoming, [bool]$Profiles) {
    $baseline = Get-UserDataBaseline $RelativePath
    $installed = Join-Path $script:mergeInstalledData $RelativePath
    $target = Join-Path $script:mergeStageData $RelativePath
    $report = [ordered]@{ file = $RelativePath; status = 'preserved_no_baseline'; conflicts = @() }
    if (-not (Test-Path -LiteralPath $installed -PathType Leaf)) {
        $report.status = 'preserved_user_deletion'
        $report.conflicts = @('File was deleted locally; incoming defaults saved separately.')
        if (Test-Path -LiteralPath $target) { Remove-Item -LiteralPath $target -Force }
    } elseif ($baseline) {
        try {
            foreach ($path in @($baseline, $installed, $Incoming)) {
                if ((Get-Item -LiteralPath $path).Length -gt 64MB) { throw 'JSON exceeds merge size limit.' }
            }
            $strictUtf8 = New-Object Text.UTF8Encoding($false, $true)
            $result = [Vsmr.Updates.JsonThreeWayMerge]::MergeJson(
                [IO.File]::ReadAllText($baseline, $strictUtf8),
                [IO.File]::ReadAllText($installed, $strictUtf8),
                [IO.File]::ReadAllText($Incoming, $strictUtf8), $Profiles)
            [IO.File]::WriteAllText($target, $result.Json + "`n", $Utf8NoBom)
            $report.status = if ($result.Conflicts.Length) { 'merged_with_conflicts' } else { 'merged' }
            $report.conflicts = @($result.Conflicts)
        } catch {
            # Never replace user data if parsing, identity matching, or writing fails.
            Copy-Item -LiteralPath $installed -Destination $target -Force
            $report.status = 'preserved_merge_error'
            $report.conflicts = @($_.Exception.Message)
        }
    }
    # Retain incoming files for conflict review and for first-upgrade migration.
    if ($report.status -ne 'merged') { Save-IncomingUserData $RelativePath $Incoming }
    Save-UserDataBaseline $RelativePath $Incoming
    $script:mergeReport.files += [pscustomobject]$report
    return [pscustomobject]$report
}

function Complete-UserDataMerge {
    $baselineRoot = Join-Path $script:mergeStageData 'UpdateBaselines'
    [IO.Directory]::CreateDirectory($baselineRoot) | Out-Null
    $index = [ordered]@{ schema_version = 1; files = $script:mergeNewHashes }
    [IO.File]::WriteAllText((Join-Path $baselineRoot 'BASELINE-HASHES.json'),
        ($index | ConvertTo-Json -Depth 5), $Utf8NoBom)
    [IO.File]::WriteAllText((Join-Path $script:mergeStageData 'DATA-UPDATE-REPORT.json'),
        ($script:mergeReport | ConvertTo-Json -Depth 8), $Utf8NoBom)
    $review = @($script:mergeReport.files | Where-Object { $_.status -ne 'merged' })
    if ($review.Count) {
        Write-Warning "$($review.Count) user data file(s) need review; user edits were retained. See vSMR_Data/DATA-UPDATE-REPORT.json and Data_Updates."
    }
}
