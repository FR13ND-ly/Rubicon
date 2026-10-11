[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$OutputDirectory,
    [string]$K6Path = 'crates/rubicon/target/pilot-tools/k6-v2.3.0/k6-v2.3.0-windows-amd64/k6.exe',
    [Parameter(Mandatory = $true)][string]$PythonPath,
    [string]$ConfigPath = 'benchmarks/diagnostics/config.json',
    [switch]$RuntimeStages,
    [switch]$Resume,
    [switch]$Smoke
)

$ErrorActionPreference = 'Stop'
if (-not $IsWindows) { throw 'This pilot runner currently requires Windows and PowerShell 7.' }
$pilotRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
Set-Location -LiteralPath $pilotRoot
$pilotOutput = [IO.Path]::GetFullPath((Join-Path $pilotRoot $OutputDirectory))
if (-not $pilotOutput.StartsWith($pilotRoot + [IO.Path]::DirectorySeparatorChar)) { throw 'Results must remain inside the repository.' }
if ($Resume -and $Smoke) { throw 'Only a complete frozen plan can be resumed.' }
if ((Test-Path -LiteralPath $pilotOutput) -and -not $Resume) { throw 'Output directory already exists; previous results are never overwritten.' }
if ($Resume -and -not (Test-Path -LiteralPath $pilotOutput)) { throw 'Resume requires an existing experiment directory.' }
$pilotConfigPath = [IO.Path]::GetFullPath((Join-Path $pilotRoot $ConfigPath))
if (-not $pilotConfigPath.StartsWith($pilotRoot + [IO.Path]::DirectorySeparatorChar)) { throw 'Configuration must remain inside the repository.' }
$pilotConfig = Get-Content -LiteralPath $pilotConfigPath -Raw | ConvertFrom-Json
if ($RuntimeStages -and $pilotConfig.runtime_stage_schema -ne 1) { throw 'Stage mode requires an explicit stage configuration.' }
if (-not $RuntimeStages -and $pilotConfig.PSObject.Properties.Name -contains 'runtime_stage_schema') { throw 'Stage configuration requires -RuntimeStages.' }
if ($Resume -and $RuntimeStages) { throw 'Stage experiments require a fresh frozen plan; unfinished attempts are preserved.' }
$pilotV8Binding = [regex]::Match((Get-Content -LiteralPath 'crates/rubicon/Cargo.lock' -Raw), '(?m)^name = "v8"\r?\nversion = "([^"]+)"').Groups[1].Value
if ($pilotV8Binding -ne $pilotConfig.embedded_v8_binding_version) { throw 'V8 binding changed; refresh the verified engine version before this experiment.' }
if ($Smoke) {
    $pilotConfig.rates_per_second = @(20)
    $pilotConfig.repetitions = 1
    $pilotConfig.warmup_seconds = 2
    $pilotConfig.settle_seconds = 1
    $pilotConfig.measurement_seconds = 3
}
$pilotRevision = (& git rev-parse HEAD).Trim()
$pilotDirty = @(& git status --porcelain)
if (-not $Smoke -and $pilotDirty.Count -ne 0) { throw 'Commit source and harness changes before the full pilot.' }
$pilotK6 = [IO.Path]::GetFullPath((Join-Path $pilotRoot $K6Path))
if (-not (Test-Path -LiteralPath $pilotK6)) { throw 'Pinned k6 executable not found; see the pilot README.' }
$pilotK6Version = (& $pilotK6 version) -join ' '
if ($LASTEXITCODE -ne 0 -or $pilotK6Version -notmatch ('v' + [regex]::Escape($pilotConfig.k6_version) + '\b')) { throw 'Unexpected k6 version.' }
$pilotNode = (Get-Command node.exe).Source
foreach ($variant in @('off', 'on')) {
    if (-not (Test-Path -LiteralPath (Join-Path $pilotRoot $pilotConfig.binary_paths.$variant))) { throw ('Missing release binary: ' + $variant) }
}
if ($RuntimeStages) {
    $stageBuildPath = [IO.Path]::GetFullPath((Join-Path $pilotRoot $pilotConfig.build_manifest))
    if (-not $stageBuildPath.StartsWith($pilotRoot + [IO.Path]::DirectorySeparatorChar)) { throw 'Build metadata must remain inside the repository.' }
    $stageBuild = Get-Content -LiteralPath $stageBuildPath -Raw | ConvertFrom-Json
    if (-not $stageBuild.application_source_revision.StartsWith($pilotConfig.application_source_revision) -or $stageBuild.profile -ne 'release') { throw 'Stage build metadata identifies a different application or profile.' }
    foreach ($stageArtifact in $stageBuild.hashes) {
        $stageArtifactPath = [IO.Path]::GetFullPath((Join-Path $pilotRoot $stageArtifact.path))
        if (-not $stageArtifactPath.StartsWith($pilotRoot + [IO.Path]::DirectorySeparatorChar)) { throw 'Build input must remain inside the repository.' }
        if ((Get-FileHash -LiteralPath $stageArtifactPath -Algorithm SHA256).Hash.ToLowerInvariant() -ne $stageArtifact.sha256) { throw ('Stage build input changed: ' + $stageArtifact.path) }
    }
}
if (-not (Test-Path -LiteralPath $PythonPath)) { throw 'Python executable not found.' }
if ([Environment]::ProcessorCount -lt 5) { throw 'The configured affinity masks require at least five logical processors.' }
if (-not $Resume) { New-Item -ItemType Directory -Path $pilotOutput | Out-Null }

function Save-PilotJson($Value, [string]$Path) {
    $Value | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $Path -Encoding utf8NoBOM
}

function Start-PilotProcess([string]$File, [string[]]$Arguments, [hashtable]$EnvironmentVariables, [long]$Affinity, [string]$LogPrefix) {
    $startInfo = [Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $File
    $startInfo.WorkingDirectory = $pilotRoot
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    foreach ($argument in $Arguments) { $startInfo.ArgumentList.Add($argument) }
    $startInfo.Environment.Remove('pm_id') | Out-Null
    foreach ($key in $EnvironmentVariables.Keys) { $startInfo.Environment[$key] = [string]$EnvironmentVariables[$key] }
    $process = [Diagnostics.Process]::Start($startInfo)
    try {
        $process.ProcessorAffinity = [IntPtr]$Affinity
        return @{
            process = $process
            stdout = $process.StandardOutput.ReadToEndAsync()
            stderr = $process.StandardError.ReadToEndAsync()
            log_prefix = $LogPrefix
        }
    } catch {
        if (-not $process.HasExited) { $process.Kill(); $process.WaitForExit() }
        $process.Dispose()
        throw
    }
}

function Stop-PilotProcess($Handle) {
    if ($null -eq $Handle) { return }
    if (-not $Handle.process.HasExited) { $Handle.process.Kill(); $Handle.process.WaitForExit() }
    [IO.File]::WriteAllText($Handle.log_prefix + '.stdout.log', $Handle.stdout.GetAwaiter().GetResult())
    [IO.File]::WriteAllText($Handle.log_prefix + '.stderr.log', $Handle.stderr.GetAwaiter().GetResult())
    $Handle.process.Dispose()
}

function Start-DiagnosticsServer([string]$Binary, [string]$Directory) {
    $handle = Start-PilotProcess $PythonPath @((Join-Path $PSScriptRoot 'console-host.py'), '--binary', $Binary, '--directory', $Directory, '--affinity', [string]$pilotConfig.server_affinity_mask, '--max-seconds', '120') @{} $pilotConfig.generator_affinity_mask (Join-Path $Directory 'console-helper')
    $deadline = [Diagnostics.Stopwatch]::StartNew()
    try {
        $processPath = Join-Path $Directory 'server-process.json'
        while (-not (Test-Path -LiteralPath $processPath)) {
            if ($handle.process.HasExited -or $deadline.Elapsed.TotalSeconds -gt 30) { throw 'Private-console server helper failed to start.' }
            Start-Sleep -Milliseconds 50
        }
        $info = Get-Content -LiteralPath $processPath -Raw | ConvertFrom-Json
        $monitor = [Diagnostics.Process]::GetProcessById($info.server_pid)
        if ($monitor.HasExited -or $monitor.MainModule.FileName -ne $Binary) { throw 'Unexpected server identity.' }
        if ($monitor.ProcessorAffinity.ToInt64() -ne $pilotConfig.server_affinity_mask) { throw 'Unexpected server affinity.' }
        $handle.server_monitor = @{ process = $monitor }
        $handle.directory = $Directory
        return $handle
    } catch {
        Set-Content -LiteralPath (Join-Path $Directory 'stop.request') -Value 'stop' -Encoding utf8NoBOM
        $handle.process.WaitForExit(20000) | Out-Null
        Stop-PilotProcess $handle
        throw
    }
}

function Stop-DiagnosticsServer($Handle, $Run) {
    Set-Content -LiteralPath (Join-Path $Handle.directory 'stop.request') -Value 'stop' -Encoding utf8NoBOM
    if (-not $Handle.process.WaitForExit(20000)) {
        $Run.cleanup_forced = $true
        if (-not $Handle.server_monitor.process.HasExited) {
            $Handle.server_monitor.process.Kill()
            $Handle.server_monitor.process.WaitForExit()
        }
        $Handle.process.WaitForExit(5000) | Out-Null
    }
    Stop-PilotProcess $Handle
    $Handle.server_monitor.process.Dispose()
    $shutdownPath = Join-Path $Handle.directory 'shutdown.json'
    if (-not (Test-Path -LiteralPath $shutdownPath)) { throw 'Missing graceful shutdown evidence.' }
    $shutdown = Get-Content -LiteralPath $shutdownPath -Raw | ConvertFrom-Json
    $Run.shutdown = $shutdown
    if ($Run.cleanup_forced -or $shutdown.forced -or $shutdown.status -ne 'completed' -or $shutdown.server_exit_code -ne 0) { throw 'Server required abnormal shutdown; diagnostic run is invalid.' }
}

function Get-PilotResponse($Client, [string]$Origin, [string]$Path, [string]$Method = 'GET') {
    $message = [Net.Http.HttpRequestMessage]::new([Net.Http.HttpMethod]::new($Method), $Origin + $Path)
    $message.Headers.TryAddWithoutValidation('Accept-Encoding', 'identity') | Out-Null
    try {
        $response = $Client.SendAsync($message).GetAwaiter().GetResult()
        try {
            $bytes = $response.Content.ReadAsByteArrayAsync().GetAwaiter().GetResult()
            $script:diagnosticsControlCompleted++
            return @{ status = [int]$response.StatusCode; body = [Text.Encoding]::UTF8.GetString($bytes); content_type = [string]$response.Content.Headers.ContentType; bytes = $bytes.Length }
        } finally { $response.Dispose() }
    } finally { $message.Dispose() }
}

function Assert-PilotPage($Response, [string]$Heading) {
    if ($Response.status -ne 200 -or $Response.content_type -notlike 'text/html*' -or -not $Response.body.Contains(('<h2>' + $Heading + '</h2>')) -or -not $Response.body.Contains('ng-server-context="ssr"') -or -not $Response.body.Contains('ngh=')) { throw ('Incorrect SSR page: ' + $Heading) }
    if ($Heading -eq 'Home' -and $Response.body -notmatch 'id="counter-value"[^>]*>\s*0\s*</span>') { throw 'Incorrect initial counter.' }
}

function Write-PilotSample($Handle, [string]$Role, $Writer) {
    if ($Handle.process.HasExited) { return }
    $Handle.process.Refresh()
    $cpu = $Handle.process.TotalProcessorTime.TotalMilliseconds.ToString('R', [Globalization.CultureInfo]::InvariantCulture)
    $Writer.WriteLine(('{0},{1},{2},{3},{4},{5},{6}' -f [datetime]::UtcNow.ToString('o'), $Handle.process.Id, $Role, $cpu, $Handle.process.WorkingSet64, $Handle.process.PrivateMemorySize64, $Handle.process.ProcessorAffinity.ToInt64()))
    $Writer.Flush()
}

$pilotHashPaths = @('crates/rubicon/Cargo.lock', 'crates/rubicon/Cargo.toml', 'crates/rubicon/src/main.rs', 'crates/rubicon/src/lib.rs', 'crates/rubicon/src/renderer.rs', 'examples/angular-demo/package-lock.json', $pilotConfig.binary_paths.off, $pilotConfig.binary_paths.on, $ConfigPath, 'benchmarks/diagnostics/run.ps1', 'benchmarks/diagnostics/console-host.py', 'benchmarks/diagnostics/analyze.py', 'examples/angular-demo/dist/rubicon/server/server.mjs', 'examples/angular-demo/dist/angular-demo/server/server.mjs', 'benchmarks/pilot/config.json', 'benchmarks/pilot/ssr.js', 'benchmarks/pilot/run.ps1', 'benchmarks/pilot/analyze.mjs', 'benchmarks/pilot/node-host.mjs', 'benchmarks/pilot/check-accounting.mjs', 'benchmarks/pilot/plot.py', 'benchmarks/pilot/preflight.json')
if ($RuntimeStages) { $pilotHashPaths += @('benchmarks/diagnostics/analyze-stages.py', 'benchmarks/diagnostics/check-stages.py', 'benchmarks/diagnostics/record-stage-build.py', 'benchmarks/diagnostics/plot-stages.py', 'benchmarks/diagnostics/stage-preflight.json', $pilotConfig.build_manifest, 'crates/rubicon/src/http_response.rs', 'crates/rubicon/src/request.rs', 'crates/rubicon/src/response.rs', 'crates/rubicon/js/web.js', 'examples/angular-demo/src/server.rubicon.ts') }
$pilotArtifacts = @($pilotHashPaths | ForEach-Object { @{ path = $_; sha256 = (Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant() } })
$pilotBrowserFiles = @('examples/angular-demo/dist/rubicon/browser', 'examples/angular-demo/dist/angular-demo/browser') | ForEach-Object {
    $directory = $_
    @(Get-ChildItem -LiteralPath $directory -File | Where-Object { $_.Extension -in @('.js', '.css', '.ico') } | ForEach-Object { @{ path = $directory + '/' + $_.Name; bytes = $_.Length; sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() } })
}
foreach ($group in ($pilotBrowserFiles | Group-Object -Property { Split-Path -Leaf $_.path })) {
    if ($group.Count -ne 2 -or @($group.Group.sha256 | Select-Object -Unique).Count -ne 1) { throw ('Browser artifacts differ between variants: ' + $group.Name) }
}
$pilotManifest = @{
    schema_version = 1; kind = $(if ($Smoke) { 'harness smoke check' } else { 'exploratory renderer diagnostic' })
    started_utc = [datetime]::UtcNow.ToString('o'); source_revision = $pilotRevision; dirty_status = $pilotDirty
    config = $pilotConfig; artifact_hashes = $pilotArtifacts; browser_artifacts = @($pilotBrowserFiles)
    build_commands = @('cargo build --release --locked --offline --jobs 4 --manifest-path crates/rubicon/Cargo.toml --bin rubicon', 'cargo build --release --locked --offline --jobs 4 --manifest-path crates/rubicon/Cargo.toml --bin rubicon --features render-timing')
    application_artifacts = 'Reused unchanged production artifacts from pilot 006; hashes recorded'
    python = (& $PythonPath --version) -join ' '
    tools = @{ k6 = $pilotK6Version; k6_executable_sha256 = (Get-FileHash -LiteralPath $pilotK6 -Algorithm SHA256).Hash.ToLowerInvariant(); rustc = (& rustc --version) -join ' '; node = ((& $pilotNode -p 'JSON.stringify({node:process.version,v8:process.versions.v8})') | ConvertFrom-Json); embedded_v8 = @{ binding = $pilotV8Binding; verified_version = $pilotConfig.embedded_v8_verified_version; evidence = 'Diagnostic linked against the release v8 rlib and native archive before the pilot' }; powershell = $PSVersionTable.PSVersion.ToString() }
    hardware = @{ processors = @(Get-CimInstance Win32_Processor | Select-Object Name,NumberOfCores,NumberOfLogicalProcessors); ram_bytes = (Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory; os = Get-CimInstance Win32_OperatingSystem | Select-Object Caption,Version,BuildNumber,OSArchitecture; power_plan = (& powercfg /getactivescheme) -join ' ' }
    correctness_scope = 'Home/About HTTP checks per run; browser checks recorded separately; no personalized-data isolation claim'
    status = 'running'
}
if ($Resume) {
    $previous = Get-Content -LiteralPath (Join-Path $pilotOutput 'manifest.json') -Raw | ConvertFrom-Json -AsHashtable
    if ($previous.status -ne 'failed') { throw 'Resume requires a preserved failed session.' }
    $mutableHarness = @('benchmarks/diagnostics/run.ps1', 'benchmarks/diagnostics/analyze.py')
    foreach ($artifact in $previous.artifact_hashes) {
        if ($artifact.path -in $mutableHarness) { continue }
        $current = @($pilotArtifacts | Where-Object { $_.path -eq $artifact.path })
        if ($current.Count -ne 1 -or $current[0].sha256 -ne $artifact.sha256) { throw ('Resume input changed: ' + $artifact.path) }
    }
    $preservedManifest = Join-Path $pilotOutput 'interrupted-manifest.json'
    if (Test-Path -LiteralPath $preservedManifest) { throw 'This runner allows one explicit continuation only.' }
    Copy-Item -LiteralPath (Join-Path $pilotOutput 'manifest.json') -Destination $preservedManifest
    $previous.resume_session = @{ source_revision = $pilotRevision; started_utc = $pilotManifest.started_utc; artifact_hashes = $pilotArtifacts; reason = 'Finish the original frozen plan while retaining internal coverage limitations; existing runs are not repeated' }
    $previous.status = 'running'
    $pilotManifest = $previous
}
Save-PilotJson $pilotManifest (Join-Path $pilotOutput 'manifest.json')
$pilotPlan = @()
for ($repetition = 1; $repetition -le $pilotConfig.repetitions; $repetition++) {
    $rates = @($pilotConfig.rates_per_second)
    if ($repetition % 2 -eq 0) { [array]::Reverse($rates) }
    for ($rateIndex = 0; $rateIndex -lt $rates.Count; $rateIndex++) {
        $variants = if (($rateIndex + $repetition) % 2 -eq 0) { @('on', 'off') } else { @('off', 'on') }
        foreach ($variant in $variants) { $pilotPlan += @{ variant = $variant; rate = $rates[$rateIndex]; repetition = $repetition } }
    }
}
if ($Resume) {
    $previousPlan = @(Get-Content -LiteralPath (Join-Path $pilotOutput 'plan.json') -Raw | ConvertFrom-Json)
    if ($previousPlan.Count -ne $pilotPlan.Count) { throw 'Resume plan length changed.' }
    for ($index = 0; $index -lt $pilotPlan.Count; $index++) {
        foreach ($field in @('variant', 'rate', 'repetition')) {
            if ($previousPlan[$index].$field -ne $pilotPlan[$index].$field) { throw 'Resume run order changed.' }
        }
    }
} else { Save-PilotJson $pilotPlan (Join-Path $pilotOutput 'plan.json') }
try {
    foreach ($case in $pilotPlan) {
        $runId = '{0}-r{1}-{2}rps' -f $case.variant, $case.repetition, $case.rate
        $runDirectory = Join-Path $pilotOutput $runId
        if (Test-Path -LiteralPath $runDirectory) {
            if (-not $Resume) { throw ('Unexpected existing run: ' + $runId) }
            $preservedRun = Get-Content -LiteralPath (Join-Path $runDirectory 'run.json') -Raw | ConvertFrom-Json
            if ($preservedRun.status -ne 'completed' -or -not (Test-Path -LiteralPath (Join-Path $runDirectory 'summary.json')) -or -not (Test-Path -LiteralPath (Join-Path $runDirectory 'diagnostics-summary.json'))) { throw ('Existing run is incomplete: ' + $runId) }
            Write-Output ('DIAGNOSTIC_RETAIN ' + $runId)
            continue
        }
        $rawDirectory = Join-Path $runDirectory 'raw'
        New-Item -ItemType Directory -Path $rawDirectory -Force | Out-Null
        $server = $null; $generator = $null; $writer = $null; $client = $null
        $script:diagnosticsControlCompleted = 0
        $run = @{ id = $runId; variant = $case.variant; rate = $case.rate; repetition = $case.repetition; measurement_seconds = $pilotConfig.measurement_seconds; config = $pilotConfig; source_revision = $pilotRevision; status = 'running' }
        if ($RuntimeStages) { $run.runtime_stage_schema = 1 }
        Save-PilotJson $run (Join-Path $runDirectory 'run.json')
        try {
            $origin = $pilotConfig.origins.($case.variant)
            $port = ([uri]$origin).Port
            if ([Net.NetworkInformation.IPGlobalProperties]::GetIPGlobalProperties().GetActiveTcpListeners() | Where-Object { $_.Port -eq $port }) { throw ('Port already occupied: ' + $port) }
            $serverFile = Join-Path $pilotRoot $pilotConfig.binary_paths.($case.variant)
            $server = Start-DiagnosticsServer $serverFile $rawDirectory
            $run.server_pid = $server.server_monitor.process.Id
            $httpHandler = [Net.Http.HttpClientHandler]::new(); $httpHandler.AllowAutoRedirect = $false
            $client = [Net.Http.HttpClient]::new($httpHandler); $client.Timeout = [timespan]::FromSeconds(2)
            if ($RuntimeStages) {
                # A flushed ready banner precedes the first HTTP request.
                # Failed startup attempts must not silently warm the application.
                $bannerWatch = [Diagnostics.Stopwatch]::StartNew()
                $bannerPath = Join-Path $rawDirectory 'server.stdout.log'
                while (-not ((Get-Content -LiteralPath $bannerPath -Raw) -like '*Rubicon is running at*')) {
                    if ($server.process.HasExited -or $bannerWatch.Elapsed.TotalSeconds -gt 30) { throw 'Server ready banner was not observed.' }
                    Start-Sleep -Milliseconds 50
                }
                $firstStarted = [datetime]::UtcNow.ToString('o')
                $firstWatch = [Diagnostics.Stopwatch]::StartNew()
                $firstResponse = Get-PilotResponse $client $origin '/'
                $firstWatch.Stop()
                $firstFinished = [datetime]::UtcNow.ToString('o')
                Assert-PilotPage $firstResponse 'Home'
                $run.first_request = @{ expected_request_id = '1'; started_utc = $firstStarted; finished_utc = $firstFinished; client_elapsed_ms = $firstWatch.Elapsed.TotalMilliseconds; status = $firstResponse.status; bytes = $firstResponse.bytes; content_gate = 'passed'; association = 'Ordered initial control request; no request-ID response header' }
                $ready = $true
            } else {
            $readiness = [Diagnostics.Stopwatch]::StartNew(); $ready = $false
            while ($readiness.Elapsed.TotalSeconds -lt 30) {
                if ($server.process.HasExited) { throw 'Server exited before readiness.' }
                try { Assert-PilotPage (Get-PilotResponse $client $origin '/') 'Home'; $ready = $true; break } catch { Start-Sleep -Milliseconds 100 }
            }
            if (-not $ready) { throw 'No correct Home response within the readiness window.' }
            }
            Assert-PilotPage (Get-PilotResponse $client $origin '/about') 'About'
            Assert-PilotPage (Get-PilotResponse $client $origin '/?sort=asc&page=2') 'Home'
            $head = Get-PilotResponse $client $origin '/' 'HEAD'
            if ($head.status -ne 200 -or $head.bytes -ne 0) { throw 'HEAD gate failed.' }
            if ((Get-PilotResponse $client $origin '/missing').status -ne 404) { throw 'Missing-route gate failed.' }
            $run.http_gate = 'passed'
            $writer = [IO.StreamWriter]::new((Join-Path $runDirectory 'resources.csv'))
            $writer.WriteLine('utc,pid,role,cpu_ms,working_set_bytes,private_bytes,affinity_mask')
            $generatorEnvironment = @{
                GOMAXPROCS = $pilotConfig.generator_logical_processors; K6_NO_USAGE_REPORT = 'true'
                PILOT_ORIGIN = $origin; PILOT_RATE = $case.rate; PILOT_SECONDS = $pilotConfig.measurement_seconds
                PILOT_WARMUP_SECONDS = $pilotConfig.warmup_seconds; PILOT_WARMUP_RATE = $pilotConfig.warmup_rate
                PILOT_SETTLE_SECONDS = $pilotConfig.settle_seconds; PILOT_VUS = $pilotConfig.measurement_vus
                PILOT_TIMEOUT_SECONDS = $pilotConfig.request_timeout_seconds; PILOT_GRACE_SECONDS = $pilotConfig.grace_seconds
                PILOT_P95_LIMIT_MS = $pilotConfig.exploratory_p95_limit_ms; PILOT_MAX_FAILURE_FRACTION = $pilotConfig.exploratory_max_failure_fraction
                PILOT_SUMMARY_PATH = Join-Path $runDirectory 'k6-summary.json'
            }
            $generator = Start-PilotProcess $pilotK6 @('run', '--quiet', '--out', ('json=' + (Join-Path $rawDirectory 'k6-points.json.gz')), (Join-Path $PSScriptRoot '../pilot/ssr.js')) $generatorEnvironment $pilotConfig.generator_affinity_mask (Join-Path $rawDirectory 'k6')
            $run.generator_pid = $generator.process.Id
            Write-Output ('PILOT_START ' + $runId)
            while (-not $generator.process.WaitForExit($pilotConfig.resource_sample_ms)) {
                if ($server.process.HasExited) { throw 'Server exited during the load window.' }
                Write-PilotSample $server.server_monitor 'server' $writer
                Write-PilotSample $generator 'generator' $writer
            }
            $run.k6_exit_code = $generator.process.ExitCode
            if ($run.k6_exit_code -notin @(0, 99)) { throw ('Unexpected k6 exit code: ' + $run.k6_exit_code) }
            if (-not (Test-Path -LiteralPath (Join-Path $runDirectory 'k6-summary.json'))) { throw 'k6 summary was not written.' }
            $writer.Dispose(); $writer = $null
            try { Assert-PilotPage (Get-PilotResponse $client $origin '/') 'Home'; Assert-PilotPage (Get-PilotResponse $client $origin '/about') 'About'; $run.recovery_ok = $true } catch { $run.recovery_ok = $false; $run.recovery_error = $_.Exception.Message }
            $run.status = 'completed'
        } catch { $run.status = 'failed'; $run.error = $_.Exception.Message; throw }
        finally {
            if ($null -ne $writer) { $writer.Dispose() }
            if ($null -ne $client) { $client.Dispose() }
            Stop-PilotProcess $generator
            $run.control_completed = $script:diagnosticsControlCompleted
            try {
                if ($null -ne $server) { Stop-DiagnosticsServer $server $run }
            } catch {
                $run.status = 'failed'
                $run.cleanup_error = $_.Exception.Message
                throw
            } finally {
                Save-PilotJson $run (Join-Path $runDirectory 'run.json')
            }
        }
        & $pilotNode (Join-Path $PSScriptRoot '../pilot/analyze.mjs') --run $runDirectory
        if ($LASTEXITCODE -ne 0) { throw ('Analysis failed: ' + $runId) }
        & $PythonPath (Join-Path $PSScriptRoot 'analyze.py') --run $runDirectory
        if ($LASTEXITCODE -notin @(0, 2)) { throw ('Diagnostic analysis process failed: ' + $runId) }
        if ($RuntimeStages) {
            & $PythonPath (Join-Path $PSScriptRoot 'analyze-stages.py') --run $runDirectory
            if ($LASTEXITCODE -notin @(0, 2)) { throw ('Stage analysis process failed: ' + $runId) }
        }
        Start-Sleep -Seconds 2
    }
    $pilotManifest.status = 'completed'
    if ($Resume) { $pilotManifest.resume_session.finished_utc = [datetime]::UtcNow.ToString('o') }
} catch { $pilotManifest.status = 'failed'; $pilotManifest.error = $_.Exception.Message; throw }
finally { $pilotManifest.finished_utc = [datetime]::UtcNow.ToString('o'); Save-PilotJson $pilotManifest (Join-Path $pilotOutput 'manifest.json') }
& $pilotNode (Join-Path $PSScriptRoot '../pilot/analyze.mjs') --all $pilotOutput
if ($LASTEXITCODE -ne 0) { throw 'Aggregate analysis failed.' }
& $PythonPath (Join-Path $PSScriptRoot 'analyze.py') --all $pilotOutput
if ($LASTEXITCODE -ne 0) { throw 'Aggregate diagnostic analysis failed.' }
if ($RuntimeStages) {
    & $PythonPath (Join-Path $PSScriptRoot 'analyze-stages.py') --all $pilotOutput
    if ($LASTEXITCODE -ne 0) { throw 'Aggregate stage analysis failed.' }
}
