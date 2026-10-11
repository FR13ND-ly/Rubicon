[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$OutputDirectory,
    [string]$K6Path = 'crates/rubicon/target/pilot-tools/k6-v2.3.0/k6-v2.3.0-windows-amd64/k6.exe',
    [switch]$Smoke
)

$ErrorActionPreference = 'Stop'
if (-not $IsWindows) { throw 'This pilot runner currently requires Windows and PowerShell 7.' }
$pilotRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
Set-Location -LiteralPath $pilotRoot
$pilotOutput = [IO.Path]::GetFullPath((Join-Path $pilotRoot $OutputDirectory))
if (-not $pilotOutput.StartsWith($pilotRoot + [IO.Path]::DirectorySeparatorChar)) { throw 'Results must remain inside the repository.' }
if (Test-Path -LiteralPath $pilotOutput) { throw 'Output directory already exists; previous results are never overwritten.' }
$pilotConfig = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'config.json') -Raw | ConvertFrom-Json
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
$pilotRust = Join-Path $pilotRoot 'crates/rubicon/target/release/rubicon.exe'
if (-not (Test-Path -LiteralPath $pilotRust)) { throw 'Build the release Rust executable before this pilot.' }
if ([Environment]::ProcessorCount -lt 5) { throw 'The configured affinity masks require at least five logical processors.' }
New-Item -ItemType Directory -Path $pilotOutput | Out-Null

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

function Get-PilotResponse($Client, [string]$Origin, [string]$Path, [string]$Method = 'GET') {
    $message = [Net.Http.HttpRequestMessage]::new([Net.Http.HttpMethod]::new($Method), $Origin + $Path)
    $message.Headers.TryAddWithoutValidation('Accept-Encoding', 'identity') | Out-Null
    try {
        $response = $Client.SendAsync($message).GetAwaiter().GetResult()
        try {
            $bytes = $response.Content.ReadAsByteArrayAsync().GetAwaiter().GetResult()
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

$pilotHashPaths = @('crates/rubicon/Cargo.lock', 'examples/angular-demo/package-lock.json', 'crates/rubicon/target/release/rubicon.exe', 'examples/angular-demo/dist/rubicon/server/server.mjs', 'examples/angular-demo/dist/angular-demo/server/server.mjs', 'benchmarks/pilot/config.json', 'benchmarks/pilot/ssr.js', 'benchmarks/pilot/run.ps1', 'benchmarks/pilot/analyze.mjs', 'benchmarks/pilot/node-host.mjs', 'benchmarks/pilot/check-accounting.mjs', 'benchmarks/pilot/plot.py', 'benchmarks/pilot/preflight.json')
$pilotArtifacts = @($pilotHashPaths | ForEach-Object { @{ path = $_; sha256 = (Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant() } })
$pilotBrowserFiles = @('examples/angular-demo/dist/rubicon/browser', 'examples/angular-demo/dist/angular-demo/browser') | ForEach-Object {
    $directory = $_
    @(Get-ChildItem -LiteralPath $directory -File | Where-Object { $_.Extension -in @('.js', '.css', '.ico') } | ForEach-Object { @{ path = $directory + '/' + $_.Name; bytes = $_.Length; sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() } })
}
foreach ($group in ($pilotBrowserFiles | Group-Object -Property { Split-Path -Leaf $_.path })) {
    if ($group.Count -ne 2 -or @($group.Group.sha256 | Select-Object -Unique).Count -ne 1) { throw ('Browser artifacts differ between variants: ' + $group.Name) }
}
$pilotManifest = @{
    schema_version = 1; kind = $(if ($Smoke) { 'harness smoke check' } else { 'exploratory pilot' })
    started_utc = [datetime]::UtcNow.ToString('o'); source_revision = $pilotRevision; dirty_status = $pilotDirty
    config = $pilotConfig; artifact_hashes = $pilotArtifacts; browser_artifacts = @($pilotBrowserFiles)
    build_commands = @('cargo build --release --locked --offline --jobs 4 --manifest-path crates/rubicon/Cargo.toml --bin rubicon', 'npm.cmd run build -- --configuration production', 'npm.cmd run build -- --configuration production,rubicon')
    tools = @{ k6 = $pilotK6Version; k6_executable_sha256 = (Get-FileHash -LiteralPath $pilotK6 -Algorithm SHA256).Hash.ToLowerInvariant(); rustc = (& rustc --version) -join ' '; node = ((& $pilotNode -p 'JSON.stringify({node:process.version,v8:process.versions.v8})') | ConvertFrom-Json); embedded_v8 = @{ binding = $pilotV8Binding; verified_version = $pilotConfig.embedded_v8_verified_version; evidence = 'Diagnostic linked against the release v8 rlib and native archive before the pilot' }; powershell = $PSVersionTable.PSVersion.ToString() }
    hardware = @{ processors = @(Get-CimInstance Win32_Processor | Select-Object Name,NumberOfCores,NumberOfLogicalProcessors); ram_bytes = (Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory; os = Get-CimInstance Win32_OperatingSystem | Select-Object Caption,Version,BuildNumber,OSArchitecture; power_plan = (& powercfg /getactivescheme) -join ' ' }
    correctness_scope = 'Home/About HTTP checks per run; browser checks recorded separately; no personalized-data isolation claim'
    status = 'running'
}
Save-PilotJson $pilotManifest (Join-Path $pilotOutput 'manifest.json')
$pilotPlan = @()
for ($repetition = 1; $repetition -le $pilotConfig.repetitions; $repetition++) {
    $rates = @($pilotConfig.rates_per_second)
    if ($repetition % 2 -eq 0) { [array]::Reverse($rates) }
    for ($rateIndex = 0; $rateIndex -lt $rates.Count; $rateIndex++) {
        $variants = if (($rateIndex + $repetition) % 2 -eq 0) { @('node', 'rubicon') } else { @('rubicon', 'node') }
        foreach ($variant in $variants) { $pilotPlan += @{ variant = $variant; rate = $rates[$rateIndex]; repetition = $repetition } }
    }
}
Save-PilotJson $pilotPlan (Join-Path $pilotOutput 'plan.json')
try {
    foreach ($case in $pilotPlan) {
        $runId = '{0}-r{1}-{2}rps' -f $case.variant, $case.repetition, $case.rate
        $runDirectory = Join-Path $pilotOutput $runId
        $rawDirectory = Join-Path $runDirectory 'raw'
        New-Item -ItemType Directory -Path $rawDirectory -Force | Out-Null
        $server = $null; $generator = $null; $writer = $null; $client = $null
        $run = @{ id = $runId; variant = $case.variant; rate = $case.rate; repetition = $case.repetition; measurement_seconds = $pilotConfig.measurement_seconds; config = $pilotConfig; source_revision = $pilotRevision; status = 'running' }
        Save-PilotJson $run (Join-Path $runDirectory 'run.json')
        try {
            $origin = $pilotConfig.origins.($case.variant)
            $port = ([uri]$origin).Port
            if ([Net.NetworkInformation.IPGlobalProperties]::GetIPGlobalProperties().GetActiveTcpListeners() | Where-Object { $_.Port -eq $port }) { throw ('Port already occupied: ' + $port) }
            $serverFile = if ($case.variant -eq 'rubicon') { $pilotRust } else { $pilotNode }
            $serverArguments = if ($case.variant -eq 'rubicon') { @() } else { @('benchmarks/pilot/node-host.mjs') }
            $server = Start-PilotProcess $serverFile $serverArguments @{} $pilotConfig.server_affinity_mask (Join-Path $rawDirectory 'server')
            $run.server_pid = $server.process.Id
            $httpHandler = [Net.Http.HttpClientHandler]::new(); $httpHandler.AllowAutoRedirect = $false
            $client = [Net.Http.HttpClient]::new($httpHandler); $client.Timeout = [timespan]::FromSeconds(2)
            $readiness = [Diagnostics.Stopwatch]::StartNew(); $ready = $false
            while ($readiness.Elapsed.TotalSeconds -lt 30) {
                if ($server.process.HasExited) { throw 'Server exited before readiness.' }
                try { Assert-PilotPage (Get-PilotResponse $client $origin '/') 'Home'; $ready = $true; break } catch { Start-Sleep -Milliseconds 100 }
            }
            if (-not $ready) { throw 'No correct Home response within the readiness window.' }
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
            $generator = Start-PilotProcess $pilotK6 @('run', '--quiet', '--out', ('json=' + (Join-Path $rawDirectory 'k6-points.json.gz')), (Join-Path $PSScriptRoot 'ssr.js')) $generatorEnvironment $pilotConfig.generator_affinity_mask (Join-Path $rawDirectory 'k6')
            $run.generator_pid = $generator.process.Id
            Write-Output ('PILOT_START ' + $runId)
            while (-not $generator.process.WaitForExit($pilotConfig.resource_sample_ms)) {
                if ($server.process.HasExited) { throw 'Server exited during the load window.' }
                Write-PilotSample $server 'server' $writer
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
            Stop-PilotProcess $server
            Save-PilotJson $run (Join-Path $runDirectory 'run.json')
        }
        & $pilotNode (Join-Path $PSScriptRoot 'analyze.mjs') --run $runDirectory
        if ($LASTEXITCODE -ne 0) { throw ('Analysis failed: ' + $runId) }
        Start-Sleep -Seconds 2
    }
    $pilotManifest.status = 'completed'
} catch { $pilotManifest.status = 'failed'; $pilotManifest.error = $_.Exception.Message; throw }
finally { $pilotManifest.finished_utc = [datetime]::UtcNow.ToString('o'); Save-PilotJson $pilotManifest (Join-Path $pilotOutput 'manifest.json') }
& $pilotNode (Join-Path $PSScriptRoot 'analyze.mjs') --all $pilotOutput
if ($LASTEXITCODE -ne 0) { throw 'Aggregate analysis failed.' }
