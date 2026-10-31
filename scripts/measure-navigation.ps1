#Requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$ImageDirectory,
    [ValidatePattern('^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$')][string]$RunName = 'navigation',
    [string]$ExecutablePath,
    [ValidateSet('Normal', 'Rapid')][string[]]$Scenario = @('Normal', 'Rapid'),
    [ValidateSet('Forward', 'Backward')][string[]]$Direction = @('Forward', 'Backward'),
    [ValidateRange(1, 10)][int]$Repetitions = 3,
    [ValidateRange(0, 3)][int]$WarmupRuns = 1,
    [ValidateRange(1, 1000)][int]$MaxImages = 100,
    [ValidateRange(10, 60000)][int]$NormalDelayMs = 250,
    [ValidateRange(10, 60000)][int]$RapidIntervalMs = 60,
    [ValidateRange(0, 60)][int]$SettleSeconds = 20,
    [ValidateRange(10, 3600)][int]$TimeoutSeconds = 600,
    [ValidateRange(20, 1000)][int]$SampleIntervalMs = 100,
    [ValidateSet('Full', 'Minimal')][string]$EventLogging = 'Full'
)

$ErrorActionPreference = 'Stop'
if (-not $IsWindows) { throw 'Navigation measurements require Windows.' }
if ($Scenario.Count -eq 0 -or $Direction.Count -eq 0) { throw 'Select at least one scenario and direction.' }
$imageRoot = (Resolve-Path -LiteralPath $ImageDirectory).Path
if (-not (Test-Path -LiteralPath $imageRoot -PathType Container)) { throw 'Select an existing image folder.' }
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$extensions = @('.jpg', '.jpeg', '.png', '.webp', '.gif', '.bmp', '.tif', '.tiff', '.heic', '.heif')
$images = @(Get-ChildItem -LiteralPath $imageRoot -File -Force | Where-Object { $_.Extension.ToLowerInvariant() -in $extensions })
if ($images.Count -eq 0) { throw 'The selected folder contains no supported images.' }

# Hash file names and contents, but never persist the names or folder path.
Write-Host "Fingerprinting $($images.Count) image files (read-only)..."
$names = [string[]]$images.Name
[Array]::Sort($names, [StringComparer]::Ordinal)
$fingerprint = [Security.Cryptography.IncrementalHash]::CreateHash([Security.Cryptography.HashAlgorithmName]::SHA256)
try {
    foreach ($name in $names) {
        $hash = (Get-FileHash -LiteralPath (Join-Path $imageRoot $name) -Algorithm SHA256).Hash
        $fingerprint.AppendData([Text.Encoding]::UTF8.GetBytes("$name`0$hash`n"))
    }
    $datasetHash = [Convert]::ToHexString($fingerprint.GetHashAndReset()).ToLowerInvariant()
} finally { $fingerprint.Dispose() }
$fileState = @($images | ForEach-Object { @{ File = $_; Length = $_.Length; Modified = $_.LastWriteTimeUtc.Ticks } })

Push-Location $projectRoot
try {
    $revision = git -c "safe.directory=$($projectRoot.Replace('\', '/'))" rev-parse HEAD
    if ($LASTEXITCODE -ne 0) { throw 'Could not identify the runner revision.' }
    $dirty = [bool](git -c "safe.directory=$($projectRoot.Replace('\', '/'))" status --porcelain)
    $builtHere = -not $ExecutablePath
    if ($builtHere) {
        $buildRoot = Join-Path $projectRoot 'target/perf/build'
        & cargo build --release --locked --features perf --target x86_64-pc-windows-msvc --target-dir $buildRoot
        if ($LASTEXITCODE -ne 0) { throw 'Measurement build failed.' }
        $ExecutablePath = Join-Path $buildRoot 'x86_64-pc-windows-msvc/release/lumen.exe'
    }
    $executable = (Resolve-Path -LiteralPath $ExecutablePath).Path
    $runId = '{0}-{1}-{2}' -f [DateTime]::UtcNow.ToString('yyyyMMddTHHmmssZ'), $RunName, [Guid]::NewGuid().ToString('N').Substring(0, 8)
    $runRoot = Join-Path $projectRoot "target/perf/runs/$runId"
    New-Item -ItemType Directory -Path (Join-Path $runRoot 'binaries') -Force | Out-Null
    $binary = Join-Path $runRoot 'binaries/lumen.exe'
    Copy-Item -LiteralPath $executable -Destination $binary
    $binaryHash = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash.ToLowerInvariant()
    @{
        run_name = $RunName; created_utc = [DateTime]::UtcNow.ToString('o')
        runner_revision = $revision; binary_revision = $(if ($builtHere) { $revision } else { $null })
        checkout_dirty = $dirty; executable_sha256 = $binaryHash
        dataset_sha256 = $datasetHash; dataset_files = $images.Count
        cache_policy = 'OS cache not flushed; input files read once for fingerprinting.'
        scenario = $Scenario; direction = $Direction; repetitions = $Repetitions; warmups = $WarmupRuns
        max_images = $MaxImages; normal_delay_ms = $NormalDelayMs; rapid_interval_ms = $RapidIntervalMs
        settle_seconds = $SettleSeconds; sample_interval_ms = $SampleIntervalMs; timeout_seconds = $TimeoutSeconds
        event_logging = $EventLogging; logical_processors = [Environment]::ProcessorCount
        os_version = [Environment]::OSVersion.VersionString
        timing_boundary = 'Frame queued by egui; not GPU completion or physical presentation.'
    } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $runRoot 'metadata.json') -Encoding utf8NoBOM

    $results = [Collections.Generic.List[object]]::new()
    function Invoke-Measurement([string]$Mode, [string]$Travel, [int]$Repeat, [bool]$Warmup) {
        $label = '{0}-{1}-{2}-{3}' -f $Mode.ToLowerInvariant(), $Travel.ToLowerInvariant(), $(if ($Warmup) { 'warmup' } else { 'run' }), $Repeat
        $directory = Join-Path $runRoot $label
        New-Item -ItemType Directory -Path $directory | Out-Null
        Write-Host "Running $label..."
        $process = [Diagnostics.Process]::new()
        $process.StartInfo.FileName = $binary
        $process.StartInfo.UseShellExecute = $false
        $process.StartInfo.CreateNoWindow = $true
        $process.StartInfo.RedirectStandardError = $true
        $interval = if ($Mode -eq 'Normal') { $NormalDelayMs } else { $RapidIntervalMs }
        foreach ($argument in @('--perf', $imageRoot, $directory, $Mode.ToLowerInvariant(), $Travel.ToLowerInvariant(),
                "$interval", "$($SettleSeconds * 1000)", "$MaxImages", "$TimeoutSeconds", $EventLogging.ToLowerInvariant())) {
            $process.StartInfo.ArgumentList.Add($argument)
        }
        $writer = [IO.StreamWriter]::new((Join-Path $directory 'memory.csv'))
        $writer.WriteLine('unix_ms,elapsed_ms,working_set_bytes,private_bytes,observed_process_peak_working_set_bytes,cpu_total_ms')
        $watch = [Diagnostics.Stopwatch]::StartNew()
        $peak = 0L; $peakPrivate = 0L; $lastWorking = 0L; $lastPrivate = 0L; $cpuMs = 0.0; $samples = 0
        $invariant = [Globalization.CultureInfo]::InvariantCulture
        $started = $false
        try {
            $process.Start() | Out-Null
            $started = $true
            $stderr = $process.StandardError.ReadToEndAsync()
            while (-not $process.HasExited) {
                if ($watch.Elapsed.TotalSeconds -gt ($TimeoutSeconds + 30)) { throw 'Measurement process timed out.' }
                if ($watch.Elapsed.TotalSeconds -gt 30 -and -not (Test-Path -LiteralPath (Join-Path $directory 'ready.json'))) {
                    throw 'No measurement handshake. Use an EXE built with the perf feature.'
                }
                try {
                    $process.Refresh()
                    $working = $process.WorkingSet64; $private = $process.PrivateMemorySize64
                    $processPeak = $process.PeakWorkingSet64
                    $sampleCpu = $process.TotalProcessorTime.TotalMilliseconds
                    if ($process.HasExited) { break }
                    $lastWorking = $working; $lastPrivate = $private
                    $peak = [Math]::Max($peak, $processPeak)
                    $peakPrivate = [Math]::Max($peakPrivate, $private)
                    $cpuMs = $sampleCpu
                    $writer.WriteLine([string]::Format($invariant, '{0},{1:F3},{2},{3},{4},{5:F3}',
                        [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds(), $watch.Elapsed.TotalMilliseconds,
                        $lastWorking, $lastPrivate, $peak, $cpuMs))
                    $samples++
                } catch [InvalidOperationException] {
                    if (-not $process.HasExited) { throw }
                }
                Start-Sleep -Milliseconds $SampleIntervalMs
            }
            $process.WaitForExit()
            $errorText = $stderr.GetAwaiter().GetResult()
            if ($process.ExitCode -ne 0) { throw "Measurement EXE failed: $errorText" }
            if ($samples -eq 0) { throw 'No memory samples were captured.' }
            $outcome = Get-Content -LiteralPath (Join-Path $directory 'outcome.json') -Raw | ConvertFrom-Json
            $row = [pscustomobject]@{
                scenario = $Mode; direction = $Travel; repetition = $Repeat; warmup = $Warmup
                status = $outcome.status; images_loaded = $outcome.images_loaded; expected_images = $outcome.expected_images
                navigation_requests = $outcome.navigation_requests; ignored_requests = $outcome.ignored_requests
                navigation_ms = $outcome.navigation_ms; mean_loader_ms = $outcome.mean_loader_ms
                mean_request_to_frame_queued_ms = $outcome.mean_request_to_frame_queued_ms
                observed_process_peak_working_set_bytes = $peak; sampled_peak_private_bytes = $peakPrivate
                last_sampled_working_set_bytes = $lastWorking; last_sampled_private_bytes = $lastPrivate
                sampled_cpu_total_ms = $cpuMs; memory_samples = $samples
            }
            $results.Add($row)
            $results | ConvertTo-Json -Depth 5 -AsArray | Set-Content -LiteralPath (Join-Path $runRoot 'runs.json') -Encoding utf8NoBOM
            $results | Export-Csv -LiteralPath (Join-Path $runRoot 'runs.csv') -NoTypeInformation -Encoding utf8NoBOM
            if ($outcome.status -ne 'completed') { throw "Run $label was $($outcome.status); it is not a valid comparison sample." }
        } finally {
            if ($started -and -not $process.HasExited) { $process.Kill(); $process.WaitForExit() }
            $writer.Dispose()
            $process.Dispose()
        }
    }

    foreach ($mode in ($Scenario | Select-Object -Unique)) {
        foreach ($travel in ($Direction | Select-Object -Unique)) {
            for ($repeat = 1; $repeat -le $WarmupRuns; $repeat++) { Invoke-Measurement $mode $travel $repeat $true }
            for ($repeat = 1; $repeat -le $Repetitions; $repeat++) { Invoke-Measurement $mode $travel $repeat $false }
        }
    }
    $after = @(Get-ChildItem -LiteralPath $imageRoot -File -Force | Where-Object { $_.Extension.ToLowerInvariant() -in $extensions })
    if ($after.Count -ne $images.Count) { throw 'The test folder changed during measurement; discard this comparison.' }
    foreach ($state in $fileState) {
        $state.File.Refresh()
        if (-not $state.File.Exists -or $state.File.Length -ne $state.Length -or $state.File.LastWriteTimeUtc.Ticks -ne $state.Modified) {
            throw 'An input file changed during measurement; discard this comparison.'
        }
    }
    function Median([double[]]$Values) {
        $ordered = @($Values | Sort-Object)
        $middle = [int][Math]::Floor($ordered.Count / 2)
        if ($ordered.Count % 2) { return $ordered[$middle] }
        return ($ordered[$middle - 1] + $ordered[$middle]) / 2
    }
    $summary = @($results | Where-Object { -not $_.warmup } | Group-Object scenario, direction | ForEach-Object {
        [pscustomobject]@{
            scenario = $_.Group[0].scenario; direction = $_.Group[0].direction; repetitions = $_.Count
            median_peak_working_set_bytes = Median $_.Group.observed_process_peak_working_set_bytes
            min_peak_working_set_bytes = ($_.Group.observed_process_peak_working_set_bytes | Measure-Object -Minimum).Minimum
            max_peak_working_set_bytes = ($_.Group.observed_process_peak_working_set_bytes | Measure-Object -Maximum).Maximum
            median_navigation_ms = Median $_.Group.navigation_ms
            median_last_sampled_private_bytes = Median $_.Group.last_sampled_private_bytes
        }
    })
    $summary | ConvertTo-Json -Depth 5 -AsArray | Set-Content -LiteralPath (Join-Path $runRoot 'summary.json') -Encoding utf8NoBOM
    $summary | Select-Object scenario, direction, repetitions,
        @{ Name = 'median_peak_mib'; Expression = { [Math]::Round($_.median_peak_working_set_bytes / 1MB, 1) } },
        @{ Name = 'median_navigation_ms'; Expression = { [Math]::Round($_.median_navigation_ms, 1) } },
        @{ Name = 'last_private_mib'; Expression = { [Math]::Round($_.median_last_sampled_private_bytes / 1MB, 1) } } |
        Format-Table -AutoSize
    Write-Host "Results: $runRoot"
} finally { Pop-Location }
