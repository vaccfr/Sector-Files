#requires -Version 5.1

[CmdletBinding(SupportsShouldProcess = $true)]
param(
    [Parameter(Mandatory = $true)]
    [string]$DestinationDirectory,
    [string]$BackupRoot = "",
    [switch]$ReplaceUserData,
    [switch]$ReloadAviso,
    [switch]$ReplaceModifiedAviso,
    [switch]$PreserveLoader,
    [switch]$RuntimeUpdate
)

$ErrorActionPreference = "Stop"
$Utf8NoBom = New-Object System.Text.UTF8Encoding($false)
$AvisoFileNamePattern = '^[A-Za-z0-9]{4}(?:_[A-Za-z0-9][A-Za-z0-9_-]{0,47})?\.geojson$'

function Resolve-NonRootDirectoryPath([string]$Path, [string]$Description) {
    if ([string]::IsNullOrWhiteSpace($Path)) { throw "$Description cannot be empty." }
    $fullPath = [System.IO.Path]::GetFullPath($Path)
    $pathRoot = [System.IO.Path]::GetPathRoot($fullPath)
    if ($fullPath.TrimEnd('\', '/').Equals($pathRoot.TrimEnd('\', '/'), [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "$Description cannot be a filesystem root: $fullPath"
    }
    return $fullPath.TrimEnd('\', '/')
}

$PackageRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\.."))
$PackageData = Join-Path $PackageRoot "vSMR_Data"
$PackageDll = Join-Path $PackageRoot "vSMR.dll"
$DestinationDirectory = Resolve-NonRootDirectoryPath $DestinationDirectory "DestinationDirectory"
$preserveTopLevelLoader = [bool]($PreserveLoader -or $RuntimeUpdate)

function Assert-File([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { throw "Required file is missing: $Path" }
}

function Get-FileSha256([string]$Path) {
    $stream = [System.IO.File]::OpenRead($Path)
    $sha256 = [System.Security.Cryptography.SHA256]::Create()
    try {
        return ([System.BitConverter]::ToString($sha256.ComputeHash($stream))).Replace('-', '').ToLowerInvariant()
    }
    finally {
        $sha256.Dispose()
        $stream.Dispose()
    }
}

function Get-CertificateDerSha256($Certificate) {
    $sha256 = [System.Security.Cryptography.SHA256]::Create()
    try {
        return ([System.BitConverter]::ToString($sha256.ComputeHash($Certificate.RawData))).Replace('-', '').ToLowerInvariant()
    }
    finally { $sha256.Dispose() }
}

function Assert-PublishableBinarySignature([string]$Path, [string]$Description, [string]$ExpectedSignerHash) {
    $signature = Get-AuthenticodeSignature -LiteralPath $Path
    if ($signature.Status -ne [System.Management.Automation.SignatureStatus]::Valid -or
        $null -eq $signature.SignerCertificate) {
        throw "Publishable package $Description does not have a valid Authenticode signature."
    }
    $actualSignerHash = Get-CertificateDerSha256 $signature.SignerCertificate
    if ($actualSignerHash -ne $ExpectedSignerHash) {
        throw "Publishable package $Description is not signed by the certificate pinned in release metadata."
    }
}

function Assert-ChildPath([string]$Path, [string]$Parent) {
    $resolvedPath = [System.IO.Path]::GetFullPath($Path).TrimEnd('\', '/')
    $resolvedParent = [System.IO.Path]::GetFullPath($Parent).TrimEnd('\', '/')
    if (-not $resolvedPath.StartsWith($resolvedParent + '\', [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Path '$resolvedPath' is outside '$resolvedParent'."
    }
}

function Test-PathEqualOrChild([string]$Path, [string]$Parent) {
    $resolvedPath = [System.IO.Path]::GetFullPath($Path).TrimEnd('\', '/')
    $resolvedParent = [System.IO.Path]::GetFullPath($Parent).TrimEnd('\', '/')
    return $resolvedPath.Equals($resolvedParent, [System.StringComparison]::OrdinalIgnoreCase) -or
        $resolvedPath.StartsWith($resolvedParent + '\', [System.StringComparison]::OrdinalIgnoreCase)
}

function Copy-DirectoryContents([string]$Source, [string]$Destination) {
    [System.IO.Directory]::CreateDirectory($Destination) | Out-Null
    foreach ($item in @(Get-ChildItem -LiteralPath $Source -Force)) {
        Copy-Item -LiteralPath $item.FullName -Destination $Destination -Recurse -Force
    }
}

function Write-TransactionOutcome([string]$Path, [string]$Status, [string]$TransactionId, [string]$RollbackBackup, [bool]$LoaderPreserved) {
    $outcome = [ordered]@{
        schema_version = 1
        kind = "vSMR install transaction outcome"
        transaction_id = $TransactionId
        status = $Status
        recorded_utc = [DateTime]::UtcNow.ToString("yyyy-MM-ddTHH:mm:ssZ")
        destination = $DestinationDirectory
        rollback_backup = $RollbackBackup
        loader_preserved = $LoaderPreserved
    }
    [System.IO.File]::WriteAllText(
        $Path,
        ((ConvertTo-Json $outcome -Depth 5) + "`n"),
        $Utf8NoBom)
}

$destinationDll = Join-Path $DestinationDirectory "vSMR.dll"
$destinationData = Join-Path $DestinationDirectory "vSMR_Data"
if (Test-PathEqualOrChild $DestinationDirectory $PackageRoot) {
    throw "DestinationDirectory cannot be the extracted package or a directory below it."
}
if (Test-PathEqualOrChild $PackageRoot $destinationData) {
    throw "The extracted package cannot be inside the vSMR_Data tree being replaced."
}
# A raw-feed transaction owns its rollback journal until recovery completes.
# Replacing the installation under that journal could later undo this manual
# installation. Check before creating backups or writing any destination data.
if (Test-Path -LiteralPath (Join-Path $destinationData '.update/journal.json')) {
    throw 'pending_file_update: finish or recover the pending vSMR update before installing a complete package.'
}

Assert-File $PackageDll
Assert-File (Join-Path $PackageData "RELEASE-METADATA.json")
Assert-File (Join-Path $PackageData "SHA256SUMS.txt")
Assert-File (Join-Path $PackageData "AVISO-UPDATE-POLICY.json")
Assert-File (Join-Path $PackageData "AVISO-INVENTORY.json")
Assert-File (Join-Path $PackageData "airports_hp.json")
Assert-File (Join-Path $PackageData "Runtime\vSMR.Runtime.dll")
Assert-File (Join-Path $PackageData "CrashReporter\vSMRCrashHandler.dll")
Assert-File (Join-Path $PackageData "Tools\restore_vsmr_backup.ps1")
Assert-File (Join-Path $PackageData "Tools\merge_user_data.ps1")
Assert-File (Join-Path $PackageData "Tools\JsonThreeWayMerge.cs")
if (-not (Test-Path -LiteralPath $DestinationDirectory -PathType Container)) {
    throw "Destination plugin directory does not exist: $DestinationDirectory"
}
# Validate every packaged payload byte before copying it into EuroScope.
$manifestEntries = @{}
foreach ($line in @(Get-Content -LiteralPath (Join-Path $PackageData "SHA256SUMS.txt"))) {
    if ([string]::IsNullOrWhiteSpace($line)) { continue }
    if ($line -notmatch '^([0-9a-f]{64})  (.+)$') { throw "The package SHA256SUMS.txt is invalid." }
    $relative = $Matches[2].Replace('/', '\')
    if ([System.IO.Path]::IsPathRooted($relative) -or @($relative.Split('\') | Where-Object { $_ -eq '..' }).Count -gt 0) {
        throw "Unsafe package manifest path: $relative"
    }
    if ($manifestEntries.ContainsKey($relative)) { throw "Duplicate package manifest path: $relative" }
    $manifestEntries[$relative] = $Matches[1]
}
foreach ($relative in @($manifestEntries.Keys)) {
    $path = Join-Path $PackageRoot $relative
    Assert-File $path
    $actual = Get-FileSha256 $path
    if ($actual -ne $manifestEntries[$relative]) { throw "Package hash mismatch: $relative" }
}
$unlisted = @(Get-ChildItem -LiteralPath $PackageRoot -Recurse -File | Where-Object {
    $_.FullName -ne (Join-Path $PackageData "SHA256SUMS.txt") -and
    -not $manifestEntries.ContainsKey($_.FullName.Substring($PackageRoot.Length).TrimStart([char[]]"\/").Replace('/', '\'))
})
if ($unlisted.Count -gt 0) { throw "Package contains unlisted files: $($unlisted.FullName -join ', ')" }

$releaseMetadata = Get-Content -LiteralPath (Join-Path $PackageData "RELEASE-METADATA.json") -Raw | ConvertFrom-Json
if ([string]$releaseMetadata.version -notmatch '^\d+\.\d+\.\d+(?:-beta\.\d+)?$') {
    throw "Package release metadata has an invalid version."
}
if ([string]$releaseMetadata.loader.relative_path -ne 'vSMR.dll' -or
    [string]$releaseMetadata.runtime.relative_path -ne 'vSMR_Data/Runtime/vSMR.Runtime.dll') {
    throw "Package release metadata has an invalid loader/runtime layout."
}

$automaticUpdateMetadata = $releaseMetadata.automatic_update
$hasPublishableMarker = $null -ne $automaticUpdateMetadata -and
    $automaticUpdateMetadata.PSObject.Properties.Name -contains 'publishable'
if ($hasPublishableMarker -and $automaticUpdateMetadata.publishable -isnot [bool]) {
    throw "Package release metadata has an invalid automatic-update publishable marker."
}
$requiresSignature = $true # Packages predating loader 1.2 used mandatory signing.
if ($null -ne $automaticUpdateMetadata -and
    $automaticUpdateMetadata.PSObject.Properties.Name -contains 'signature_required') {
    if ($automaticUpdateMetadata.signature_required -isnot [bool]) {
        throw 'Invalid package signature policy.'
    }
    $requiresSignature = [bool]$automaticUpdateMetadata.signature_required
}
if ($hasPublishableMarker -and [bool]$automaticUpdateMetadata.publishable -and $requiresSignature) {
    $expectedSignerHash = ([string]$automaticUpdateMetadata.signer_cert_der_sha256).ToLowerInvariant()
    if ($expectedSignerHash -notmatch '^[0-9a-f]{64}$') {
        throw "Publishable package release metadata has an invalid signer certificate pin."
    }

    # Automatic updates authenticate the outer archive. A manual installation
    # reaches this script directly, so enforce the same pinned signer here too.
    Assert-PublishableBinarySignature $PackageDll "loader" $expectedSignerHash
    Assert-PublishableBinarySignature (Join-Path $PackageData "Runtime\vSMR.Runtime.dll") "runtime" $expectedSignerHash
    Assert-PublishableBinarySignature (Join-Path $PackageData "CrashReporter\vSMRCrashHandler.dll") "crash handler" $expectedSignerHash
}

$avisoPolicy = Get-Content -LiteralPath (Join-Path $PackageData "AVISO-UPDATE-POLICY.json") -Raw | ConvertFrom-Json
$avisoInventory = Get-Content -LiteralPath (Join-Path $PackageData "AVISO-INVENTORY.json") -Raw | ConvertFrom-Json
$supportedAvisoUpdates = @('none', 'selected', 'all')
$supportedModifiedFilePolicies = @('preserve', 'protect_setting', 'replace')
if ([int]$avisoPolicy.schema_version -ne 1 -or
    [string]$avisoPolicy.release -ne [string]$releaseMetadata.version -or
    $avisoPolicy.aviso -isnot [pscustomobject] -or
    $supportedAvisoUpdates -notcontains [string]$avisoPolicy.aviso.update -or
    $supportedModifiedFilePolicies -notcontains [string]$avisoPolicy.aviso.modified_files -or
    $avisoPolicy.aviso.replace -isnot [System.Array] -or
    $avisoPolicy.aviso.delete -isnot [System.Array] -or
    [int]$avisoInventory.schema_version -ne 1 -or
    [string]$avisoInventory.release -ne [string]$releaseMetadata.version -or
    $null -eq $avisoInventory.files) {
    throw "The packaged AVISO update policy or inventory is invalid."
}

function Assert-AvisoFileName([string]$Name) {
    if ([string]::IsNullOrWhiteSpace($Name) -or
        [System.IO.Path]::IsPathRooted($Name) -or
        [System.IO.Path]::GetFileName($Name) -ne $Name -or
        $Name.IndexOfAny([System.IO.Path]::GetInvalidFileNameChars()) -ge 0 -or
        $Name -notmatch $AvisoFileNamePattern) {
        throw "Unsafe AVISO update filename: $Name"
    }
}

function ConvertTo-AvisoHashTable($Inventory) {
    $result = @{}
    if ($null -eq $Inventory -or $null -eq $Inventory.files) { return $result }
    foreach ($property in @($Inventory.files.PSObject.Properties)) {
        $result[[string]$property.Name] = ([string]$property.Value).ToLowerInvariant()
    }
    return $result
}

function Read-LegacyAvisoHashTable([string]$ManifestPath) {
    $result = @{}
    if (-not (Test-Path -LiteralPath $ManifestPath -PathType Leaf)) { return $result }
    foreach ($line in @(Get-Content -LiteralPath $ManifestPath)) {
        if ($line -match '^([0-9a-fA-F]{64})  vSMR_Data[\\/]+AVISO[\\/]+([^\\/]+\.geojson)$') {
            $name = [string]$Matches[2]
            try { Assert-AvisoFileName $name }
            catch { continue }
            $result[$name] = ([string]$Matches[1]).ToLowerInvariant()
        }
    }
    return $result
}

$policyReplaceNames = @($avisoPolicy.aviso.replace | ForEach-Object { [string]$_ })
$policyDeleteNames = @($avisoPolicy.aviso.delete | ForEach-Object { [string]$_ })
foreach ($name in @($policyReplaceNames) + @($policyDeleteNames)) {
    Assert-AvisoFileName ([string]$name)
}
$duplicatePolicyNames = @($policyReplaceNames + $policyDeleteNames |
    Group-Object { $_.ToLowerInvariant() } | Where-Object { $_.Count -gt 1 })
if ($duplicatePolicyNames.Count -gt 0) {
    throw "The AVISO update policy contains duplicate or overlapping filenames."
}
if ([string]$avisoPolicy.aviso.update -eq 'selected' -and $policyReplaceNames.Count -eq 0) {
    throw "A selected AVISO update policy must name at least one replacement."
}
if ([string]$avisoPolicy.aviso.update -ne 'selected' -and $policyReplaceNames.Count -gt 0) {
    throw "Only a selected AVISO update policy may contain replacement filenames."
}

$packageInventoryTable = ConvertTo-AvisoHashTable $avisoInventory
foreach ($property in @($avisoInventory.files.PSObject.Properties)) {
    Assert-AvisoFileName ([string]$property.Name)
    if ([string]$property.Value -notmatch '^[0-9a-fA-F]{64}$') {
        throw "Invalid AVISO inventory hash for $($property.Name)."
    }
    $inventoryFile = Join-Path $PackageData "AVISO\$($property.Name)"
    Assert-File $inventoryFile
    $inventoryHash = Get-FileSha256 $inventoryFile
    if ($inventoryHash -ne ([string]$property.Value).ToLowerInvariant()) {
        throw "AVISO inventory hash mismatch for $($property.Name)."
    }
}
$packagedAvisoFiles = @(Get-ChildItem -LiteralPath (Join-Path $PackageData 'AVISO') -Filter '*.geojson' -File)
if ($packagedAvisoFiles.Count -ne $packageInventoryTable.Count) {
    throw "The AVISO inventory does not describe every packaged GeoJSON file."
}
foreach ($file in $packagedAvisoFiles) {
    if (-not $packageInventoryTable.ContainsKey($file.Name)) {
        throw "Packaged AVISO is missing from the inventory: $($file.Name)"
    }
}
foreach ($name in $policyReplaceNames) {
    if (-not $packageInventoryTable.ContainsKey($name)) {
        throw "Selected AVISO replacement is not bundled: $name"
    }
}
foreach ($name in $policyDeleteNames) {
    if ($packageInventoryTable.ContainsKey($name)) {
        throw "An AVISO cannot be both bundled and scheduled for deletion: $name"
    }
}

if ([string]::IsNullOrWhiteSpace($BackupRoot)) {
    $BackupRoot = Join-Path $DestinationDirectory "vSMR_Backups"
}
$BackupRoot = Resolve-NonRootDirectoryPath $BackupRoot "BackupRoot"
if (Test-PathEqualOrChild $BackupRoot $destinationData) {
    throw "BackupRoot cannot be inside the vSMR_Data directory being replaced."
}
if (Test-PathEqualOrChild $BackupRoot $PackageRoot) {
    throw "BackupRoot cannot be inside the extracted package."
}
$stamp = [DateTime]::UtcNow.ToString("yyyyMMdd_HHmmss")
$backupDirectory = Join-Path $BackupRoot ("vSMR-before-" + $releaseMetadata.version + "-" + $stamp + "-" + [Guid]::NewGuid().ToString("N").Substring(0, 8))
Assert-ChildPath $backupDirectory $BackupRoot
$hadDll = Test-Path -LiteralPath $destinationDll -PathType Leaf
$hadData = Test-Path -LiteralPath $destinationData -PathType Container
if ($preserveTopLevelLoader -and -not $hadDll) {
    throw "Runtime-update mode requires an existing top-level vSMR.dll loader."
}
$packageLoaderHash = Get-FileSha256 $PackageDll
$packageLoaderVersion = [string]$releaseMetadata.loader.version
if ([string]::IsNullOrWhiteSpace($packageLoaderVersion)) {
    $packageLoaderVersion = [string](([System.Diagnostics.FileVersionInfo]::GetVersionInfo($PackageDll)).FileVersion)
}
$installedLoaderHash = if ($preserveTopLevelLoader) {
    Get-FileSha256 $destinationDll
} else { $packageLoaderHash }
$installedLoaderSize = if ($preserveTopLevelLoader) {
    [int64](Get-Item -LiteralPath $destinationDll).Length
} else {
    [int64](Get-Item -LiteralPath $PackageDll).Length
}
$installedLoaderVersion = if ($preserveTopLevelLoader) {
    [string](([System.Diagnostics.FileVersionInfo]::GetVersionInfo($destinationDll)).FileVersion)
} else { $packageLoaderVersion }
$minimumLoaderVersionText = [string]$releaseMetadata.automatic_update.minimum_loader_version
$minimumLoaderVersion = $null
$parsedInstalledLoaderVersion = $null
if (-not [Version]::TryParse($minimumLoaderVersionText, [ref]$minimumLoaderVersion)) {
    throw "Package release metadata has an invalid minimum loader version."
}
if ($preserveTopLevelLoader -and
    (-not [Version]::TryParse($installedLoaderVersion, [ref]$parsedInstalledLoaderVersion) -or
        $parsedInstalledLoaderVersion -lt $minimumLoaderVersion)) {
    throw "manual_loader_update_required: installed loader '$installedLoaderVersion' is older than required '$minimumLoaderVersionText'; run a complete manual installation without -PreserveLoader."
}
$loaderMatchesPackage = $installedLoaderHash -eq $packageLoaderHash

# Confirm before creating even a backup so -WhatIf is genuinely non-mutating.
$installMode = if ($preserveTopLevelLoader) { "runtime/data update while preserving the loaded vSMR.dll loader" } else { "complete loader/runtime update" }
if (-not $PSCmdlet.ShouldProcess($DestinationDirectory, "Install vSMR $($releaseMetadata.version) as a $installMode; create rollback backup under $BackupRoot")) {
    Write-Host "Installation skipped; no files were changed."
    return
}

[System.IO.Directory]::CreateDirectory($BackupRoot) | Out-Null
[System.IO.Directory]::CreateDirectory($backupDirectory) | Out-Null
if ($hadDll) { Copy-Item -LiteralPath $destinationDll -Destination $backupDirectory }
if ($hadData) { Copy-Item -LiteralPath $destinationData -Destination $backupDirectory -Recurse }

$profileSchemas = @()
if ($hadData -and (Test-Path -LiteralPath (Join-Path $destinationData "vSMR_Profiles.json") -PathType Leaf)) {
    try {
        $oldProfiles = Get-Content -LiteralPath (Join-Path $destinationData "vSMR_Profiles.json") -Raw | ConvertFrom-Json
        $profileSchemas = @($oldProfiles | Where-Object { $_.PSObject.Properties.Name -contains 'name' } |
            ForEach-Object { [int]$_.schema_version } | Sort-Object -Unique)
    }
    catch { $profileSchemas = @("unreadable") }
}
$backupMetadata = [ordered]@{
    schema_version = 1
    kind = "vSMR complete pre-install backup"
    created_utc = [DateTime]::UtcNow.ToString("yyyy-MM-ddTHH:mm:ssZ")
    destination = $DestinationDirectory
    installing_version = [string]$releaseMetadata.version
    had_dll = [bool]$hadDll
    had_data = [bool]$hadData
    loader_preserved = $preserveTopLevelLoader
    profile_schema_versions = $profileSchemas
}
[System.IO.File]::WriteAllText(
    (Join-Path $backupDirectory "BACKUP-METADATA.json"),
    ((ConvertTo-Json $backupMetadata -Depth 10) + "`n"),
    $Utf8NoBom)

$operationId = [Guid]::NewGuid().ToString("N")
$transactionOutcomePath = Join-Path $backupDirectory "TRANSACTION-OUTCOME.json"
Write-TransactionOutcome $transactionOutcomePath "prepared" $operationId $backupDirectory $preserveTopLevelLoader
$stageRoot = Join-Path $DestinationDirectory (".vsmr-install-" + $operationId)
$oldDataSwap = Join-Path $DestinationDirectory (".vsmr-previous-data-" + $operationId)
$oldDllSwap = Join-Path $DestinationDirectory (".vsmr-previous-dll-" + $operationId)
Assert-ChildPath $stageRoot $DestinationDirectory
Assert-ChildPath $oldDataSwap $DestinationDirectory
Assert-ChildPath $oldDllSwap $DestinationDirectory
[System.IO.Directory]::CreateDirectory($stageRoot) | Out-Null
$stageData = Join-Path $stageRoot "vSMR_Data"
Copy-Item -LiteralPath $PackageData -Destination $stageData -Recurse
$stageDll = Join-Path $stageRoot "vSMR.dll"
if (-not $preserveTopLevelLoader) { Copy-Item -LiteralPath $PackageDll -Destination $stageDll }

if ($hadData -and -not $ReplaceUserData) {
    $immutableNames = @(
        'vSMR_webUI', 'CrashReporter', 'Licenses', 'Runtime', 'Tools',
        'RELEASE-METADATA.json', 'SHA256SUMS.txt', 'INSTALLATION.json',
        'AVISO-UPDATE-POLICY.json', 'AVISO-INVENTORY.json', 'AVISO-UPDATE-REPORT.json',
        'airports_hp.json', 'AVISO', 'UpdateBaselines', 'DATA-UPDATE-REPORT.json',
        'default.json', 'version.json', '.update'
    )
    foreach ($item in @(Get-ChildItem -LiteralPath $destinationData -Force)) {
        if ($immutableNames -contains $item.Name) { continue }
        $target = Join-Path $stageData $item.Name
        if ($item.PSIsContainer) {
            Copy-DirectoryContents $item.FullName $target
        }
        else {
            Copy-Item -LiteralPath $item.FullName -Destination $target -Force
        }
    }
}

. (Join-Path $PSScriptRoot 'merge_user_data.ps1')
Initialize-UserDataMerge $destinationData $stageData ([string]$releaseMetadata.version)
$profilesName = 'vSMR_Profiles.json'
$incomingProfiles = Join-Path $PackageData $profilesName
if (Test-Path -LiteralPath $incomingProfiles -PathType Leaf) {
    if ($hadData -and -not $ReplaceUserData) {
        $installedProfiles = Join-Path $destinationData $profilesName
        $profileBaseline = Get-UserDataBaseline $profilesName
        # An old package hash can prove the entire profile file is unmodified.
        $unchangedProfiles = $false
        $oldChecksums = Join-Path $destinationData 'SHA256SUMS.txt'
        if ((Test-Path -LiteralPath $installedProfiles -PathType Leaf) -and (Test-Path -LiteralPath $oldChecksums -PathType Leaf)) {
            foreach ($line in Get-Content -LiteralPath $oldChecksums) {
                if ($line -match '^([0-9a-f]{64})  vSMR_Data[\\/]vSMR_Profiles\.json$') {
                    $unchangedProfiles = (Get-FileSha256 $installedProfiles) -eq $Matches[1]
                    break
                }
            }
        }
        if ($unchangedProfiles) {
            Copy-Item -LiteralPath $incomingProfiles -Destination (Join-Path $stageData $profilesName) -Force
        } elseif ((Test-Path -LiteralPath $installedProfiles -PathType Leaf) -or $profileBaseline) {
            $null = Merge-UserDataFile $profilesName $incomingProfiles $true
        }
    }
    Save-UserDataBaseline $profilesName $incomingProfiles
}

$avisoReport = [ordered]@{
    schema_version = 1
    release = [string]$releaseMetadata.version
    operation = if ($ReloadAviso) { 'manual_reload' } else { 'release_update' }
    policy = if ($ReloadAviso) { 'all' } else { [string]$avisoPolicy.aviso.update }
    modified_file_policy = if ($ReloadAviso) { 'protect_setting' } else { [string]$avisoPolicy.aviso.modified_files }
    protected_modified_files = $false
    baseline_source = 'none'
    updated = @()
    added = @()
    preserved_modified = @()
    merged = @()
    merge_conflicts = @()
    preserved_user_deleted = @()
    deleted = @()
    preserved_deleted_modified = @()
    custom_preserved = @()
}

if ($hadData -and -not $ReplaceUserData) {
    $installedAviso = Join-Path $destinationData 'AVISO'
    $stageAviso = Join-Path $stageData 'AVISO'
    if (Test-Path -LiteralPath $stageAviso) {
        Remove-Item -LiteralPath $stageAviso -Recurse -Force
    }
    [System.IO.Directory]::CreateDirectory($stageAviso) | Out-Null
    if (Test-Path -LiteralPath $installedAviso -PathType Container) {
        Copy-DirectoryContents $installedAviso $stageAviso
    }

    $oldBaseline = @{}
    $oldInventoryPath = Join-Path $destinationData 'AVISO-INVENTORY.json'
    if (Test-Path -LiteralPath $oldInventoryPath -PathType Leaf) {
        try {
            $candidateInventory = Get-Content -LiteralPath $oldInventoryPath -Raw | ConvertFrom-Json
            if ([int]$candidateInventory.schema_version -eq 1 -and $null -ne $candidateInventory.files) {
                foreach ($property in @($candidateInventory.files.PSObject.Properties)) {
                    Assert-AvisoFileName ([string]$property.Name)
                    if ([string]$property.Value -notmatch '^[0-9a-fA-F]{64}$') {
                        throw "Invalid installed AVISO inventory entry."
                    }
                }
                $oldBaseline = ConvertTo-AvisoHashTable $candidateInventory
                $avisoReport.baseline_source = 'inventory'
            }
        }
        catch {
            $oldBaseline = @{}
        }
    }
    if ($oldBaseline.Count -eq 0) {
        $oldBaseline = Read-LegacyAvisoHashTable (Join-Path $destinationData 'SHA256SUMS.txt')
        if ($oldBaseline.Count -gt 0) { $avisoReport.baseline_source = 'legacy_sha256sums' }
    }
    $effectiveBaseline = @{}
    foreach ($name in @($oldBaseline.Keys)) {
        if (Test-Path -LiteralPath (Join-Path $stageAviso $name) -PathType Leaf) {
            $effectiveBaseline[$name] = [string]$oldBaseline[$name]
        }
    }

    $replaceNames = @()
    $effectiveUpdate = if ($ReloadAviso) { 'all' } else { [string]$avisoPolicy.aviso.update }
    if ($effectiveUpdate -eq 'all') {
        $replaceNames = @($avisoInventory.files.PSObject.Properties | ForEach-Object { [string]$_.Name })
    }
    elseif ($effectiveUpdate -eq 'selected') {
        $replaceNames = @($policyReplaceNames)
    }

    $modifiedPolicy = if ($ReloadAviso) { 'protect_setting' } else { [string]$avisoPolicy.aviso.modified_files }
    $protectModified = $modifiedPolicy -eq 'preserve' -or
        ($modifiedPolicy -eq 'protect_setting' -and -not [bool]$ReplaceModifiedAviso)
    $avisoReport.protected_modified_files = [bool]$protectModified
    $incomingDirectory = Join-Path $stageData ("AVISO_Updates\" + [string]$releaseMetadata.version)

    foreach ($name in @($replaceNames | Sort-Object -Unique)) {
        Assert-AvisoFileName $name
        $packageFile = Join-Path $PackageData "AVISO\$name"
        Assert-File $packageFile
        $installedFile = Join-Path $installedAviso $name
        $stageFile = Join-Path $stageAviso $name
        $installedExists = Test-Path -LiteralPath $installedFile -PathType Leaf
        $relative = 'AVISO/' + $name
        if (-not $installedExists -and $protectModified -and
            ($oldBaseline.ContainsKey($name) -or (Get-UserDataBaseline $relative))) {
            $null = Merge-UserDataFile $relative $packageFile $false
            $avisoReport.preserved_user_deleted += $name
            continue
        }
        $modified = $false
        if ($installedExists) {
            $modified = -not $oldBaseline.ContainsKey($name)
            if (-not $modified) {
                $installedHash = Get-FileSha256 $installedFile
                $modified = $installedHash -ne [string]$oldBaseline[$name]
            }
        }

        if ($installedExists -and $modified -and $protectModified) {
            [System.IO.Directory]::CreateDirectory($incomingDirectory) | Out-Null
            Copy-Item -LiteralPath $packageFile -Destination (Join-Path $incomingDirectory $name) -Force
            $merge = Merge-UserDataFile $relative $packageFile $false
            if ($merge.status -like 'merged*') {
                $avisoReport.merged += $name
                $effectiveBaseline[$name] = [string]$packageInventoryTable[$name]
                if ($merge.conflicts.Count) { $avisoReport.merge_conflicts += $name }
            } else { $avisoReport.preserved_modified += $name }
            continue
        }

        [System.IO.Directory]::CreateDirectory($stageAviso) | Out-Null
        Copy-Item -LiteralPath $packageFile -Destination $stageFile -Force
        $effectiveBaseline[$name] = [string]$packageInventoryTable[$name]
        Save-UserDataBaseline $relative $packageFile
        if ($installedExists) { $avisoReport.updated += $name } else { $avisoReport.added += $name }
    }

    foreach ($name in @($policyDeleteNames | Sort-Object -Unique)) {
        Assert-AvisoFileName $name
        $installedFile = Join-Path $installedAviso $name
        $stageFile = Join-Path $stageAviso $name
        if (-not (Test-Path -LiteralPath $stageFile -PathType Leaf)) { continue }

        $modified = -not $oldBaseline.ContainsKey($name)
        if (-not $modified -and (Test-Path -LiteralPath $installedFile -PathType Leaf)) {
            $installedHash = Get-FileSha256 $installedFile
            $modified = $installedHash -ne [string]$oldBaseline[$name]
        }
        if ($modified -and $protectModified) {
            $avisoReport.preserved_deleted_modified += $name
            continue
        }
        Remove-Item -LiteralPath $stageFile -Force
        $effectiveBaseline.Remove($name)
        $script:mergeNewHashes.Remove('AVISO/' + $name)
        $avisoReport.deleted += $name
    }

    $knownNames = @{}
    foreach ($name in @($packageInventoryTable.Keys) + @($oldBaseline.Keys) + @($policyDeleteNames)) {
        $knownNames[[string]$name] = $true
    }
    if (Test-Path -LiteralPath $installedAviso -PathType Container) {
        $avisoReport.custom_preserved = @(Get-ChildItem -LiteralPath $installedAviso -Filter '*.geojson' -File |
            Where-Object { -not $knownNames.ContainsKey($_.Name) } |
            ForEach-Object { $_.Name } | Sort-Object)
    }

    $effectiveInventoryFiles = [ordered]@{}
    foreach ($name in @($effectiveBaseline.Keys | Sort-Object)) {
        if (Test-Path -LiteralPath (Join-Path $stageAviso $name) -PathType Leaf) {
            $effectiveInventoryFiles[$name] = [string]$effectiveBaseline[$name]
        }
    }
    $effectiveInventory = [ordered]@{
        schema_version = 1
        release = [string]$releaseMetadata.version
        files = $effectiveInventoryFiles
    }
    [System.IO.File]::WriteAllText(
        (Join-Path $stageData 'AVISO-INVENTORY.json'),
        ((ConvertTo-Json $effectiveInventory -Depth 5) + "`n"),
        $Utf8NoBom)
}
else {
    $avisoReport.protected_modified_files = $false
    $avisoReport.added = @($avisoInventory.files.PSObject.Properties | ForEach-Object { [string]$_.Name })
    foreach ($name in $avisoReport.added) {
        Save-UserDataBaseline ('AVISO/' + $name) (Join-Path $PackageData ('AVISO/' + $name))
    }
}

Complete-UserDataMerge

[System.IO.File]::WriteAllText(
    (Join-Path $stageData 'AVISO-UPDATE-REPORT.json'),
    ((ConvertTo-Json $avisoReport -Depth 8) + "`n"),
    $Utf8NoBom)

$installationMetadata = [ordered]@{
    schema_version = 1
    product = "vSMR"
    installed_version = [string]$releaseMetadata.version
    installed_utc = [DateTime]::UtcNow.ToString("yyyy-MM-ddTHH:mm:ssZ")
    source_commit = [string]$releaseMetadata.git_commit
    user_data_preserved = -not [bool]$ReplaceUserData
    aviso_reload = [bool]$ReloadAviso
    aviso_modified_files_protected = [bool]$avisoReport.protected_modified_files
    loader_preserved = $preserveTopLevelLoader
    validation_scope = if ($preserveTopLevelLoader) { "runtime_and_data" } else { "full_package" }
    package_loader_version = $packageLoaderVersion
    package_loader_sha256 = $packageLoaderHash
    installed_loader_version = $installedLoaderVersion
    installed_loader_relative_path = "vSMR.dll"
    installed_loader_size = $installedLoaderSize
    installed_loader_sha256 = $installedLoaderHash
    loader_matches_package = $loaderMatchesPackage
    rollback_backup = $backupDirectory
    transaction_id = $operationId
}
[System.IO.File]::WriteAllText(
    (Join-Path $stageData "INSTALLATION.json"),
    ((ConvertTo-Json $installationMetadata -Depth 10) + "`n"),
    $Utf8NoBom)

$dataSwapped = $false
$dllSwapped = $false
$transactionCommitted = $false
try {
    if ($hadData) { Move-Item -LiteralPath $destinationData -Destination $oldDataSwap }
    Move-Item -LiteralPath $stageData -Destination $destinationData
    $dataSwapped = $true
    if (-not $preserveTopLevelLoader) {
        if ($hadDll) { Move-Item -LiteralPath $destinationDll -Destination $oldDllSwap }
        Move-Item -LiteralPath $stageDll -Destination $destinationDll
        $dllSwapped = $true
    }
    Write-TransactionOutcome $transactionOutcomePath "committed" $operationId $backupDirectory $preserveTopLevelLoader
    $transactionCommitted = $true
}
catch {
    $installError = $_
    try {
        if ($dllSwapped -and (Test-Path -LiteralPath $destinationDll)) { Remove-Item -LiteralPath $destinationDll -Force }
        if (Test-Path -LiteralPath $oldDllSwap) { Move-Item -LiteralPath $oldDllSwap -Destination $destinationDll }
        if ($dataSwapped -and (Test-Path -LiteralPath $destinationData)) { Remove-Item -LiteralPath $destinationData -Recurse -Force }
        if (Test-Path -LiteralPath $oldDataSwap) { Move-Item -LiteralPath $oldDataSwap -Destination $destinationData }
        Write-TransactionOutcome $transactionOutcomePath "rolled_back" $operationId $backupDirectory $preserveTopLevelLoader
    }
    catch {
        Write-Warning "Install rollback or transaction-journal update was incomplete: $($_.Exception.Message)"
    }
    throw $installError
}
finally {
    try {
        if (Test-Path -LiteralPath $stageRoot) { Remove-Item -LiteralPath $stageRoot -Recurse -Force }
    }
    catch {
        Write-Warning "vSMR was not changed by staging cleanup, but temporary files remain at '$stageRoot': $($_.Exception.Message)"
    }
}

if (-not $transactionCommitted) {
    throw "The install transaction did not reach its durable committed outcome."
}
foreach ($obsoleteSwap in @($oldDllSwap, $oldDataSwap)) {
    try {
        if (Test-Path -LiteralPath $obsoleteSwap) {
            Remove-Item -LiteralPath $obsoleteSwap -Recurse -Force
        }
    }
    catch {
        Write-Warning "vSMR was installed successfully, but obsolete pre-swap files could not be removed from '$obsoleteSwap': $($_.Exception.Message)"
    }
}
Write-Host "Installed vSMR $($releaseMetadata.version)."
if ($preserveTopLevelLoader) { Write-Host "Preserved the loaded top-level vSMR.dll loader." }
Write-Host "Rollback backup: $backupDirectory"
