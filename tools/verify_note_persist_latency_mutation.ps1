# v0.0.1 - Execute cancellable save admission and transaction-bound preflight mutations.
param([string]$Version)
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if (!$Version) { $Version = [regex]::Match([IO.File]::ReadAllText((Join-Path $root 'app/build.gradle')), "versionName '([^']+)'").Groups[1].Value }
if ($Version -notmatch '^\d+\.\d+\.\d+\.\d+$') { throw 'Invalid Android version' }
$evidence = Join-Path $root "release_artifacts/verification/v$Version/note_persist_latency_mutation"
$generatedRoot = Join-Path $root 'app/build/generated/source/rustAndroid/main/com/ofairyo/gridtimer/data'
$sourceRoot = Join-Path $root 'native/gridtimer_native/src/sourcegen'
$utf8 = [Text.UTF8Encoding]::new($false)
function Normalize-LatencyLines([string]$Text) { $Text.Replace("`r`n", "`n") }
function Read-LatencyLiteral([string]$Text, [string]$Name) {
    $match = [regex]::Match($Text, '(?s)(?:pub\s+)?const\s+' + [regex]::Escape($Name) + ':\s*&str\s*=\s*r(?<hash>#+)"(?<body>.*?)"\k<hash>;')
    if (!$match.Success) { throw "Missing Rust source literal: $Name" }
    Normalize-LatencyLines $match.Groups['body'].Value
}
function Replace-LatencyMutation([string]$Text, [string]$Before, [string]$After, [string]$Name) {
    if ([regex]::Matches($Text, [regex]::Escape($Before)).Count -ne 1) { throw "Missing or repeated latency mutation anchor: $Name" }
    $Text.Replace($Before, $After)
}
$sources = @(
    @{name='debounce';file='android_note_persist_debounce.rs';literal='CONTENTS';helper='NotePersistDebounce.kt';test='NotePersistDebounceTest.kt';suite='com.ofairyo.gridtimer.data.NotePersistDebounceTest'},
    @{name='preflight';file='android_note_persist_preflight.rs';literal='POLICY';helper='NoteSavePreflight.kt';test='NoteSavePreflightTest.kt';suite='com.ofairyo.gridtimer.data.NoteSavePreflightTest'}
)
$identities = @()
foreach ($source in $sources) {
    $path = Join-Path $sourceRoot $source.file
    $raw = [IO.File]::ReadAllText($path)
    $source.production = Read-LatencyLiteral $raw $source.literal
    $source.tests = Read-LatencyLiteral $raw 'TEST_CONTENTS'
    $source.count = [regex]::Matches($source.tests, '@Test\s+fun ').Count
    if ($source.count -lt 1) { throw "Save latency domain tests are missing: $($source.name)" }
    $identities += [ordered]@{path=$path;sha256=(Get-FileHash -LiteralPath $path).Hash.ToLowerInvariant()}
}
$databasePath = Join-Path $generatedRoot 'AppStateDatabase.kt'
$repositoryPath = Join-Path $generatedRoot 'TimerRepository.kt'
$database = Normalize-LatencyLines ([IO.File]::ReadAllText($databasePath))
$repository = Normalize-LatencyLines ([IO.File]::ReadAllText($repositoryPath))
$windowStart = $database.IndexOf('internal class SnapshotVerificationWindow {')
$windowEnd = $database.IndexOf('private object NativeSnapshotVerifier {', $windowStart)
if ($windowStart -lt 0 -or $windowEnd -le $windowStart) { throw 'Generated snapshot verification window is missing' }
$window = $database.Substring($windowStart, $windowEnd - $windowStart).Trim()
if (!$database.Contains($sources[1].production.Trim())) { throw 'Generated save preflight differs from the actual Rust policy' }
$generatedDebounce = Join-Path $generatedRoot $sources[0].helper
if ((Normalize-LatencyLines ([IO.File]::ReadAllText($generatedDebounce))).Trim() -cne $sources[0].production.Trim()) { throw 'Generated debounce helper differs from its Rust source' }
foreach ($binding in @('val succeeded = withCancellableNotePersistLock(writeMutex) {', '): Int? = stateDatabase.withVerifiedSavePreflight(workspaceKey) {', 'skipVerifiedHistory = { evidence.acceptedCurrentWithoutFuture }')) {
    if (!$repository.Contains($binding)) { throw "Production repository is not using the latency boundary: $binding" }
}
foreach ($binding in @('fun <T> withVerifiedSavePreflight(workspaceKey: String, action: () -> T): T =', 'transactionActive = { writableDatabase.inTransaction() }', 'snapshotStamp = ::snapshotWriteStamp', 'verifyRows = { verifyNativeSnapshotRows(workspaceKey) }')) {
    if (!$database.Contains($binding)) { throw "Production database is not using the preflight boundary: $binding" }
}
$editorPath = Join-Path $root 'app/build/generated/source/rustAndroid/main/com/ofairyo/gridtimer/ui/NoteDocumentEditor.kt'
$editor = Normalize-LatencyLines ([IO.File]::ReadAllText($editorPath))
$backStart = $editor.IndexOf('    fun handleBack()')
$backEnd = $editor.IndexOf('    fun navigateHierarchy(', $backStart)
if ($backStart -lt 0 -or $backEnd -le $backStart) { throw 'Generated editor exit handler is missing' }
$back = $editor.Substring($backStart, $backEnd - $backStart)
$lastBinding = -1
foreach ($binding in @('if (!completion.accepted) return@exitCompletion', 'if (!completion.notifyUi) return@exitCompletion', 'if (completion.savedLocally) {', 'hierarchyHandedOff = true', 'editorSessionViewModel.clear(note.id, editorWorkspaceKey, editorSession)', 'onBack()')) {
    $position = $back.IndexOf($binding)
    if ($position -le $lastBinding) { throw "Successful exit suppression is outside its accepted save boundary: $binding" }
    $lastBinding = $position
}
if ([regex]::Matches($back, 'hierarchyHandedOff = true').Count -ne 1) { throw 'Exit handoff must occur exactly once in the successful exit branch' }
foreach ($binding in @('if (hierarchyHandedOff) { onComplete(true); return }', 'if (hierarchyHandedOff || isSavingDraft || pendingRevisionRestore != null) return@LaunchedEffect')) {
    if (!$editor.Contains($binding)) { throw "Disposed or leaving editor can enqueue duplicate persistence: $binding" }
}
foreach ($path in @($databasePath, $repositoryPath, $generatedDebounce, $editorPath, (Join-Path $sourceRoot 'android_note_save_queue.rs'))) {
    $identities += [ordered]@{path=$path;sha256=(Get-FileHash -LiteralPath $path).Hash.ToLowerInvariant()}
}
$cache = Join-Path $env:USERPROFILE '.gradle/caches/modules-2/files-2.1'
function Find-LatencyDependency([string]$Coordinate, [string]$DependencyVersion, [string]$Name) {
    $files = @(Get-ChildItem -LiteralPath (Join-Path $cache ($Coordinate + '/' + $DependencyVersion)) -File -Recurse | Where-Object Name -eq $Name)
    if ($files.Count -ne 1) { throw "Missing or ambiguous installed dependency: $Name" }
    $files[0].FullName
}
$compiler = @(
    (Find-LatencyDependency 'org.jetbrains.kotlin/kotlin-compiler-embeddable' '1.9.24' 'kotlin-compiler-embeddable-1.9.24.jar'),
    (Find-LatencyDependency 'org.jetbrains.kotlin/kotlin-stdlib' '1.9.24' 'kotlin-stdlib-1.9.24.jar'),
    (Find-LatencyDependency 'org.jetbrains.kotlin/kotlin-script-runtime' '1.9.24' 'kotlin-script-runtime-1.9.24.jar'),
    (Find-LatencyDependency 'org.jetbrains.kotlin/kotlin-reflect' '1.6.10' 'kotlin-reflect-1.6.10.jar'),
    (Find-LatencyDependency 'org.jetbrains.kotlin/kotlin-daemon-embeddable' '1.9.24' 'kotlin-daemon-embeddable-1.9.24.jar'),
    (Find-LatencyDependency 'org.jetbrains.intellij.deps/trove4j' '1.0.20200330' 'trove4j-1.0.20200330.jar'),
    (Find-LatencyDependency 'org.jetbrains/annotations' '13.0' 'annotations-13.0.jar')
)
$runtime = @(
    (Find-LatencyDependency 'org.jetbrains.kotlin/kotlin-stdlib' '1.9.24' 'kotlin-stdlib-1.9.24.jar'),
    (Find-LatencyDependency 'org.jetbrains.kotlinx/kotlinx-coroutines-core-jvm' '1.8.1' 'kotlinx-coroutines-core-jvm-1.8.1.jar'),
    (Find-LatencyDependency 'junit/junit' '4.13.2' 'junit-4.13.2.jar'),
    (Find-LatencyDependency 'org.hamcrest/hamcrest-core' '1.3' 'hamcrest-core-1.3.jar'),
    (Find-LatencyDependency 'org.jetbrains/annotations' '23.0.0' 'annotations-23.0.0.jar')
)
$java = 'C:\tools\java\jdk-17.0.18+8\bin\java.exe'
function Invoke-LatencyJava([string[]]$Arguments, [string]$LogPath, [int]$TimeoutSeconds) {
    $start = [Diagnostics.ProcessStartInfo]::new($java)
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    foreach ($argument in $Arguments) { $start.ArgumentList.Add($argument) }
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $start
    try {
        if (!$process.Start()) { throw 'Cannot start isolated mutation JVM' }
        $stdout = $process.StandardOutput.ReadToEndAsync()
        $stderr = $process.StandardError.ReadToEndAsync()
        if (!$process.WaitForExit($TimeoutSeconds * 1000)) {
            $process.Kill($true)
            $process.WaitForExit()
            [IO.File]::WriteAllText($LogPath, $stdout.GetAwaiter().GetResult() + $stderr.GetAwaiter().GetResult(), $utf8)
            throw "Isolated mutation JVM timed out after $TimeoutSeconds seconds: $LogPath"
        }
        [IO.File]::WriteAllText($LogPath, $stdout.GetAwaiter().GetResult() + $stderr.GetAwaiter().GetResult(), $utf8)
        $process.ExitCode
    } finally { $process.Dispose() }
}
$results = @()
$cases = @('baseline', 'cosmetic_only', 'noncancellable_waiter', 'cancelled_admitted_transaction', 'removed_database_stamp_guard', 'removed_native_success_guard', 'removed_transaction_guard', 'removed_future_schema_guard')
foreach ($case in $cases) {
    $directory = Join-Path $evidence $case
    New-Item -ItemType Directory -Force -Path $directory | Out-Null
    $debounce = $sources[0].production
    $preflight = $sources[1].production
    switch ($case) {
        'cosmetic_only' { $preflight = Replace-LatencyMutation $preflight 'Save preflight requires the primary transaction.' 'This save check requires an active database transaction.' $case }
        'noncancellable_waiter' { $debounce = Replace-LatencyMutation $debounce "): T = writeMutex.withLock {`n    withContext(NonCancellable) { commit() }`n}" "): T = withContext(NonCancellable) {`n    writeMutex.withLock { commit() }`n}" $case }
        'cancelled_admitted_transaction' { $debounce = Replace-LatencyMutation $debounce 'withContext(NonCancellable) { commit() }' 'commit()' $case }
        'removed_database_stamp_guard' { $preflight = Replace-LatencyMutation $preflight 'if (verifiedRows >= 0L && before == after) {' 'if (verifiedRows >= 0L) {' $case }
        'removed_native_success_guard' { $preflight = Replace-LatencyMutation $preflight 'if (verifiedRows >= 0L && before == after) {' 'if (before == after) {' $case }
        'removed_transaction_guard' { $preflight = Replace-LatencyMutation $preflight 'check(transactionActive()) { "Save preflight requires the primary transaction." }' '// Transaction guard removed.' $case }
        'removed_future_schema_guard' { $preflight = Replace-LatencyMutation $preflight 'if (effective > supportedVersion) {' 'if (false) {' $case }
    }
    $sourceFiles = @()
    foreach ($source in $sources) {
        $production = if ($source.name -eq 'debounce') { $debounce } else { "package com.ofairyo.gridtimer.data`n" + $window + "`n" + $preflight }
        $helperPath = Join-Path $directory $source.helper
        $testPath = Join-Path $directory $source.test
        [IO.File]::WriteAllText($helperPath, $production, $utf8)
        [IO.File]::WriteAllText($testPath, $source.tests, $utf8)
        $sourceFiles += @($helperPath, $testPath)
    }
    $classes = Join-Path $directory ('classes_' + [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds())
    $compileLog = Join-Path $directory 'compile.log'
    $compileExit = Invoke-LatencyJava (@('-Xmx512m', '-cp', ($compiler -join ';'), 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler', '-no-stdlib', '-no-reflect', '-jvm-target', '17', '-classpath', ($runtime -join ';'), '-d', $classes) + $sourceFiles) $compileLog 120
    if ($compileExit -ne 0) { Get-Content -LiteralPath $compileLog -Tail 30; throw "Mutation compilation failed: $case" }
    $testLog = Join-Path $directory 'tests.log'
    $exitCode = Invoke-LatencyJava (@('-Xmx256m', '-cp', ($classes + ';' + ($runtime -join ';')), 'org.junit.runner.JUnitCore') + @($sources | ForEach-Object {$_.suite})) $testLog 30
    $log = [IO.File]::ReadAllText($testLog)
    $testCount = ($sources | Measure-Object -Property count -Sum).Sum
    $expectedPass = $case -in @('baseline', 'cosmetic_only')
    $requiredFailure = switch ($case) {
        'noncancellable_waiter' { 'supersededWaiterCannotStartAnotherSnapshotWrite' }
        'cancelled_admitted_transaction' { 'cancellationAfterAdmissionKeepsTransactionAndLockUntilCommitCompletes' }
        'removed_database_stamp_guard' { 'changedDatabaseStampRejectsNativeReceipt' }
        'removed_native_success_guard' { 'failedOrUnavailableNativeVerificationCannotReuseAnOuterReceipt' }
        'removed_transaction_guard' { 'noTransactionCannotVerifyOrPublishAReceipt' }
        'removed_future_schema_guard' { 'futureQuarantineAndMirrorEvidenceStillBlockSaving' }
        default { '' }
    }
    $passed = if ($expectedPass) { $exitCode -eq 0 -and $log.Contains("OK ($testCount tests)") } else { $exitCode -ne 0 -and $log.Contains('java.lang.AssertionError') -and $log.Contains($requiredFailure) }
    $results += [ordered]@{case=$case;passed=$passed;expectedPass=$expectedPass;compileExit=$compileExit;testExit=$exitCode;tests=$testCount;requiredFailure=$requiredFailure;compileLogSha256=(Get-FileHash -LiteralPath $compileLog).Hash.ToLowerInvariant();testLogSha256=(Get-FileHash -LiteralPath $testLog).Hash.ToLowerInvariant();files=@($sourceFiles | ForEach-Object { [ordered]@{name=[IO.Path]::GetFileName($_);sha256=(Get-FileHash -LiteralPath $_).Hash.ToLowerInvariant()} })}
    if (!$passed) { Get-Content -LiteralPath $testLog -Tail 35; throw "Unexpected latency mutation result: $case" }
    Write-Output "$case verified"
}
foreach ($identity in $identities) {
    if ((Get-FileHash -LiteralPath $identity.path).Hash.ToLowerInvariant() -cne $identity.sha256) { throw "Production changed during save latency mutation: $($identity.path)" }
    $identity.path = $identity.path.Substring($root.Length + 1).Replace('\', '/')
}
[ordered]@{passed=$true;version=$Version;tests=$testCount;productionUnchanged=$true;productionIntegrationVerified=$true;generatedChecked=$true;identities=$identities;suites=@($sources | ForEach-Object {[ordered]@{name=$_.suite;tests=$_.count}});cases=$results;hostOnly=$true;deviceVerified=$false} | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $evidence 'receipt.json') -Encoding utf8
