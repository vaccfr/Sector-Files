#requires -Version 5.1

[CmdletBinding(SupportsShouldProcess = $true)]
param(
    [Parameter(Mandatory = $true)][string]$DestinationDirectory,
    [Parameter(Mandatory = $true)][string]$BackupDirectory,
    [switch]$PreserveLoader,
    [switch]$RuntimeUpdate
)

$ErrorActionPreference = "Stop"
function Resolve-NonRootDirectoryPath([string]$Path, [string]$Description) {
    if ([string]::IsNullOrWhiteSpace($Path)) { throw "$Description cannot be empty." }
    $fullPath = [System.IO.Path]::GetFullPath($Path)
    $pathRoot = [System.IO.Path]::GetPathRoot($fullPath)
    if ($fullPath.TrimEnd('\', '/').Equals($pathRoot.TrimEnd('\', '/'), [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "$Description cannot be a filesystem root: $fullPath"
    }
    return $fullPath.TrimEnd('\', '/')
}

$DestinationDirectory = Resolve-NonRootDirectoryPath $DestinationDirectory "DestinationDirectory"
$BackupDirectory = Resolve-NonRootDirectoryPath $BackupDirectory "BackupDirectory"
$preserveTopLevelLoader = [bool]($PreserveLoader -or $RuntimeUpdate)
function Write-RollbackOutcome([string]$Path, [string]$Status, [string]$TransactionId, [string]$SourceBackup, [bool]$LoaderPreserved) {
    $outcome = [ordered]@{
        schema_version = 1
        kind = "vSMR rollback transaction outcome"
        transaction_id = $TransactionId
        status = $Status
        recorded_utc = [DateTime]::UtcNow.ToString("yyyy-MM-ddTHH:mm:ssZ")
        destination = $DestinationDirectory
        source_backup = $SourceBackup
        loader_preserved = $LoaderPreserved
    }
    [System.IO.File]::WriteAllText(
        $Path,
        ((ConvertTo-Json $outcome -Depth 5) + "`n"),
        (New-Object System.Text.UTF8Encoding($false)))
}
function Test-PathEqualOrChild([string]$Path, [string]$Parent) {
    $resolvedPath = [System.IO.Path]::GetFullPath($Path).TrimEnd('\', '/')
    $resolvedParent = [System.IO.Path]::GetFullPath($Parent).TrimEnd('\', '/')
    return $resolvedPath.Equals($resolvedParent, [System.StringComparison]::OrdinalIgnoreCase) -or
        $resolvedPath.StartsWith($resolvedParent + '\', [System.StringComparison]::OrdinalIgnoreCase)
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

function Get-PeMachine([string]$Path, [string]$Description) {
    $stream = [System.IO.File]::OpenRead($Path)
    $reader = New-Object System.IO.BinaryReader($stream)
    try {
        if ($reader.ReadUInt16() -ne 0x5A4D) { throw "$Description has no DOS/PE header." }
        $stream.Position = 0x3C
        $peOffset = $reader.ReadInt32()
        if ($peOffset -lt 0 -or $peOffset -gt ($stream.Length - 6)) { throw "$Description has an invalid PE offset." }
        $stream.Position = $peOffset
        if ($reader.ReadUInt32() -ne 0x00004550) { throw "$Description has an invalid PE signature." }
        return $reader.ReadUInt16()
    }
    finally {
        $reader.Dispose()
        $stream.Dispose()
    }
}

$destinationDataPath = [System.IO.Path]::GetFullPath((Join-Path $DestinationDirectory "vSMR_Data")).TrimEnd('\', '/')
$backupDataPath = [System.IO.Path]::GetFullPath((Join-Path $BackupDirectory "vSMR_Data")).TrimEnd('\', '/')
if ((Test-Path -LiteralPath (Join-Path $destinationDataPath '.update/journal.json')) -or
    (Test-Path -LiteralPath (Join-Path $backupDataPath '.update/journal.json'))) {
    throw 'pending_file_update: finish or recover the file-update transaction before restoring a complete backup.'
}
if ((Test-PathEqualOrChild $BackupDirectory $destinationDataPath) -or
    (Test-PathEqualOrChild $destinationDataPath $backupDataPath)) {
    throw "Backup and active vSMR_Data trees cannot overlap: $BackupDirectory"
}
$metadataPath = Join-Path $BackupDirectory "BACKUP-METADATA.json"
if (-not (Test-Path -LiteralPath $DestinationDirectory -PathType Container)) { throw "Destination does not exist." }
if (-not (Test-Path -LiteralPath $metadataPath -PathType Leaf)) { throw "Backup metadata is missing." }
$metadata = Get-Content -LiteralPath $metadataPath -Raw | ConvertFrom-Json
$requiredMetadataFields = @('schema_version', 'kind', 'destination', 'had_dll', 'had_data')
foreach ($field in $requiredMetadataFields) {
    if ($metadata.PSObject.Properties.Name -notcontains $field) {
        throw "Backup metadata is missing required field '$field'."
    }
}
if (($metadata.schema_version -isnot [int] -and $metadata.schema_version -isnot [long]) -or
    [int64]$metadata.schema_version -ne 1 -or
    $metadata.kind -isnot [string] -or [string]::IsNullOrWhiteSpace($metadata.kind) -or
    $metadata.destination -isnot [string] -or [string]::IsNullOrWhiteSpace($metadata.destination) -or
    $metadata.had_dll -isnot [bool] -or
    $metadata.had_data -isnot [bool]) {
    throw "Backup metadata contains invalid field types or values."
}
$supportedKinds = @('vSMR complete pre-install backup', 'vSMR pre-rollback safety backup')
if ($supportedKinds -notcontains $metadata.kind) {
    throw "This is not a supported vSMR backup."
}
$metadataDestination = Resolve-NonRootDirectoryPath $metadata.destination "Backup metadata destination"
if (-not $metadataDestination.Equals($DestinationDirectory, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "This backup belongs to '$metadataDestination', not '$DestinationDirectory'."
}
if ($metadata.had_dll -and -not (Test-Path -LiteralPath (Join-Path $BackupDirectory "vSMR.dll") -PathType Leaf)) {
    throw "Backup metadata expects vSMR.dll, but the file is missing."
}
if ($metadata.had_data -and -not (Test-Path -LiteralPath (Join-Path $BackupDirectory "vSMR_Data") -PathType Container)) {
    throw "Backup metadata expects vSMR_Data, but the directory is missing."
}
$hasBackupReleaseMetadata = $false
$backupReleaseMetadata = $null
$hasBackupInstallationMetadata = $false
$backupInstallationMetadata = $null
if ($metadata.had_data) {
    $backupData = Join-Path $BackupDirectory "vSMR_Data"
    $backupReleaseMetadataPath = Join-Path $backupData "RELEASE-METADATA.json"
    $backupInstallationMetadataPath = Join-Path $backupData "INSTALLATION.json"
    $backupRuntimePath = Join-Path $backupData "Runtime\vSMR.Runtime.dll"
    $hasBackupReleaseMetadata = Test-Path -LiteralPath $backupReleaseMetadataPath -PathType Leaf
    $hasBackupInstallationMetadata = Test-Path -LiteralPath $backupInstallationMetadataPath -PathType Leaf
    $hasBackupRuntime = Test-Path -LiteralPath $backupRuntimePath -PathType Leaf
    if ($preserveTopLevelLoader -and (-not $hasBackupReleaseMetadata -or -not $hasBackupRuntime)) {
        throw "Backup does not contain a verifiable canonical vSMR runtime."
    }
    if ($hasBackupReleaseMetadata -ne $hasBackupRuntime) {
        throw "Backup contains an incomplete canonical runtime/metadata pair."
    }
    if ($hasBackupInstallationMetadata -and (-not $hasBackupReleaseMetadata -or -not $hasBackupRuntime)) {
        throw "Backup installation metadata has no canonical release/runtime pair."
    }
    if ($hasBackupRuntime) {
        $backupReleaseMetadata = Get-Content -LiteralPath $backupReleaseMetadataPath -Raw | ConvertFrom-Json
        $expectedRuntimeHash = ([string]$backupReleaseMetadata.runtime.sha256).ToLowerInvariant()
        $expectedRuntimeSize = [int64]$backupReleaseMetadata.runtime.size
        if ($backupReleaseMetadata.product -ne 'vSMR' -or
            [string]$backupReleaseMetadata.runtime.relative_path -ne 'vSMR_Data/Runtime/vSMR.Runtime.dll' -or
            $expectedRuntimeSize -le 0 -or
            $expectedRuntimeHash -notmatch '^[0-9a-f]{64}$' -or
            [int64](Get-Item -LiteralPath $backupRuntimePath).Length -ne $expectedRuntimeSize -or
            (Get-FileSha256 $backupRuntimePath) -ne $expectedRuntimeHash -or
            (Get-PeMachine $backupRuntimePath "Backup runtime") -ne 0x014C) {
            throw "Backup canonical runtime failed metadata, size, SHA-256, or Win32 validation."
        }
    }
    if ($hasBackupInstallationMetadata) {
        $backupInstallationMetadata = Get-Content -LiteralPath $backupInstallationMetadataPath -Raw | ConvertFrom-Json
    }
}

if (-not $preserveTopLevelLoader -and $metadata.had_dll) {
    $backupLoaderPath = Join-Path $BackupDirectory "vSMR.dll"
    $backupLoaderSize = [int64](Get-Item -LiteralPath $backupLoaderPath).Length
    $backupLoaderHash = Get-FileSha256 $backupLoaderPath
    if ($backupLoaderSize -le 0 -or
        (Get-PeMachine $backupLoaderPath "Backup loader") -ne 0x014C) {
        throw "Backup loader is not a Win32 vSMR DLL."
    }

    if ($hasBackupInstallationMetadata) {
        $installationFields = @($backupInstallationMetadata.PSObject.Properties.Name)
        $installedLoaderHash = ([string]$backupInstallationMetadata.installed_loader_sha256).ToLowerInvariant()
        $packageLoaderHash = ([string]$backupInstallationMetadata.package_loader_sha256).ToLowerInvariant()
        $hasInstalledLoaderSize = $installationFields -contains 'installed_loader_size'
        $hasInstalledLoaderPath = $installationFields -contains 'installed_loader_relative_path'
        if (($backupInstallationMetadata.schema_version -isnot [int] -and
                $backupInstallationMetadata.schema_version -isnot [long]) -or
            [int64]$backupInstallationMetadata.schema_version -ne 1 -or
            $backupInstallationMetadata.product -isnot [string] -or
            [string]$backupInstallationMetadata.product -ne 'vSMR' -or
            $backupInstallationMetadata.installed_loader_version -isnot [string] -or
            [string]::IsNullOrWhiteSpace([string]$backupInstallationMetadata.installed_loader_version) -or
            $backupInstallationMetadata.package_loader_version -isnot [string] -or
            [string]::IsNullOrWhiteSpace([string]$backupInstallationMetadata.package_loader_version) -or
            $backupInstallationMetadata.loader_preserved -isnot [bool] -or
            $backupInstallationMetadata.loader_matches_package -isnot [bool] -or
            $installedLoaderHash -notmatch '^[0-9a-f]{64}$' -or
            $packageLoaderHash -notmatch '^[0-9a-f]{64}$' -or
            ($hasInstalledLoaderPath -and
                [string]$backupInstallationMetadata.installed_loader_relative_path -ne 'vSMR.dll') -or
            ($hasInstalledLoaderSize -and
                (($backupInstallationMetadata.installed_loader_size -isnot [int] -and
                    $backupInstallationMetadata.installed_loader_size -isnot [long]) -or
                    [int64]$backupInstallationMetadata.installed_loader_size -le 0))) {
            throw "Backup installation metadata contains invalid loader identity fields."
        }
        if ($backupLoaderHash -ne $installedLoaderHash -or
            ($hasInstalledLoaderSize -and
                $backupLoaderSize -ne [int64]$backupInstallationMetadata.installed_loader_size)) {
            throw "Backup loader failed installed-loader size or SHA-256 validation."
        }
        if ([bool]$backupInstallationMetadata.loader_matches_package -ne
                ($installedLoaderHash -eq $packageLoaderHash) -or
            (-not [bool]$backupInstallationMetadata.loader_preserved -and
                -not [bool]$backupInstallationMetadata.loader_matches_package)) {
            throw "Backup installation metadata has inconsistent loader state."
        }

        # A runtime-only update deliberately keeps the installed loader, so its
        # bytes may differ from the loader recorded in the new release package.
        if ($hasBackupReleaseMetadata) {
            $expectedLoaderHash = ([string]$backupReleaseMetadata.loader.sha256).ToLowerInvariant()
            $expectedLoaderSize = [int64]$backupReleaseMetadata.loader.size
            if ([string]$backupReleaseMetadata.loader.relative_path -ne 'vSMR.dll' -or
                $expectedLoaderHash -notmatch '^[0-9a-f]{64}$' -or
                $expectedLoaderSize -le 0 -or
                $packageLoaderHash -ne $expectedLoaderHash) {
                throw "Backup installation metadata does not match its recorded release package."
            }
            if ([bool]$backupInstallationMetadata.loader_matches_package -and
                ($backupLoaderHash -ne $expectedLoaderHash -or
                    $backupLoaderSize -ne $expectedLoaderSize)) {
                throw "Backup loader does not match its recorded release package."
            }
        }
    }
    elseif ($hasBackupReleaseMetadata) {
        # Legacy installs did not record the actual installed-loader identity.
        # Their release metadata is authoritative only because the loader was
        # not yet independently preserved across runtime-only updates.
        $expectedLoaderHash = ([string]$backupReleaseMetadata.loader.sha256).ToLowerInvariant()
        $expectedLoaderSize = [int64]$backupReleaseMetadata.loader.size
        if ([string]$backupReleaseMetadata.loader.relative_path -ne 'vSMR.dll' -or
            $expectedLoaderSize -le 0 -or
            $expectedLoaderHash -notmatch '^[0-9a-f]{64}$' -or
            $backupLoaderSize -ne $expectedLoaderSize -or
            $backupLoaderHash -ne $expectedLoaderHash) {
            throw "Backup loader failed release metadata, size, or SHA-256 validation."
        }
    }
}
$destinationDll = Join-Path $DestinationDirectory "vSMR.dll"
$destinationData = Join-Path $DestinationDirectory "vSMR_Data"
$currentDll = Test-Path -LiteralPath $destinationDll -PathType Leaf
$currentData = Test-Path -LiteralPath $destinationData -PathType Container
if ($preserveTopLevelLoader -and -not $currentDll) {
    throw "Runtime-update rollback requires an existing top-level vSMR.dll loader."
}
$restoreMode = if ($preserveTopLevelLoader) { " while preserving the loaded top-level vSMR.dll loader" } else { "" }
if (-not $PSCmdlet.ShouldProcess($DestinationDirectory, "Restore vSMR backup $BackupDirectory$restoreMode")) { return }

$safetyRoot = Join-Path (Split-Path -Parent $BackupDirectory) ("vSMR-before-rollback-" + [DateTime]::UtcNow.ToString("yyyyMMdd_HHmmss") + "-" + [Guid]::NewGuid().ToString("N").Substring(0, 8))
[System.IO.Directory]::CreateDirectory($safetyRoot) | Out-Null
if ($currentDll) { Copy-Item -LiteralPath $destinationDll -Destination $safetyRoot }
if ($currentData) { Copy-Item -LiteralPath $destinationData -Destination $safetyRoot -Recurse }
$safetyMetadata = [ordered]@{
    schema_version = 1
    kind = "vSMR pre-rollback safety backup"
    created_utc = [DateTime]::UtcNow.ToString("yyyy-MM-ddTHH:mm:ssZ")
    destination = $DestinationDirectory
    source_backup = $BackupDirectory
    had_dll = [bool]$currentDll
    had_data = [bool]$currentData
    loader_preserved = $preserveTopLevelLoader
}
[System.IO.File]::WriteAllText(
    (Join-Path $safetyRoot "BACKUP-METADATA.json"),
    ((ConvertTo-Json $safetyMetadata -Depth 5) + "`n"),
    (New-Object System.Text.UTF8Encoding($false)))

$operationId = [Guid]::NewGuid().ToString("N")
$rollbackOutcomePath = Join-Path $safetyRoot "TRANSACTION-OUTCOME.json"
Write-RollbackOutcome $rollbackOutcomePath "prepared" $operationId $BackupDirectory $preserveTopLevelLoader
$oldDll = Join-Path $DestinationDirectory (".vsmr-rollback-dll-" + $operationId)
$oldData = Join-Path $DestinationDirectory (".vsmr-rollback-data-" + $operationId)
$dllMoved = $false
$dataMoved = $false
$newDllInstallAttempted = $false
$newDataInstallAttempted = $false
$rollbackCommitted = $false
try {
    if (-not $preserveTopLevelLoader -and $currentDll) { Move-Item -LiteralPath $destinationDll -Destination $oldDll; $dllMoved = $true }
    if ($currentData) { Move-Item -LiteralPath $destinationData -Destination $oldData; $dataMoved = $true }
    if (-not $preserveTopLevelLoader -and $metadata.had_dll) {
		$newDllInstallAttempted = $true
        Copy-Item -LiteralPath (Join-Path $BackupDirectory "vSMR.dll") -Destination $destinationDll
    }
    if ($metadata.had_data) {
		$newDataInstallAttempted = $true
        Copy-Item -LiteralPath (Join-Path $BackupDirectory "vSMR_Data") -Destination $destinationData -Recurse
    }
    Write-RollbackOutcome $rollbackOutcomePath "committed" $operationId $BackupDirectory $preserveTopLevelLoader
    $rollbackCommitted = $true
}
catch {
	$restoreError = $_
    try {
		if ($newDllInstallAttempted -and (Test-Path -LiteralPath $destinationDll)) { Remove-Item -LiteralPath $destinationDll -Force }
		if ($newDataInstallAttempted -and (Test-Path -LiteralPath $destinationData)) { Remove-Item -LiteralPath $destinationData -Recurse -Force }
        if ($dllMoved -and (Test-Path -LiteralPath $oldDll)) { Move-Item -LiteralPath $oldDll -Destination $destinationDll }
        if ($dataMoved -and (Test-Path -LiteralPath $oldData)) { Move-Item -LiteralPath $oldData -Destination $destinationData }
        Write-RollbackOutcome $rollbackOutcomePath "rolled_back" $operationId $BackupDirectory $preserveTopLevelLoader
    }
    catch {
        Write-Warning "Rollback recovery or transaction-journal update was incomplete: $($_.Exception.Message)"
    }
    throw $restoreError
}
if (-not $rollbackCommitted) {
    throw "The rollback transaction did not reach its durable committed outcome."
}
foreach ($obsoleteSwap in @($oldDll, $oldData)) {
    try {
        if (Test-Path -LiteralPath $obsoleteSwap) {
            Remove-Item -LiteralPath $obsoleteSwap -Recurse -Force
        }
    }
    catch {
        Write-Warning "The rollback succeeded, but obsolete pre-swap files could not be removed from '$obsoleteSwap': $($_.Exception.Message)"
    }
}
Write-Host "Restored vSMR backup: $BackupDirectory"
if ($preserveTopLevelLoader) { Write-Host "Preserved the loaded top-level vSMR.dll loader." }
Write-Host "Pre-rollback safety backup: $safetyRoot"
