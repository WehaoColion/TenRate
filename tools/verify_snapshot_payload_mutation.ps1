# v0.0.1 - Exercise the real snapshot byte reader and its integrity mutations.
param([string]$Version)
$ErrorActionPreference = 'Stop'
$taskRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if (!$Version) { $Version = [regex]::Match([IO.File]::ReadAllText((Join-Path $taskRoot 'app/build.gradle')), "versionName '([^']+)'").Groups[1].Value }
if ($Version -notmatch '^\d+\.\d+\.\d+\.\d+$') { throw 'Invalid Android version' }
$taskSourcePath = Join-Path $taskRoot 'native/gridtimer_native/src/sourcegen/android_snapshot_row_read.rs'
$taskSource = [IO.File]::ReadAllText($taskSourcePath)
function Read-RowReadLiteral([string]$Name) {
    $taskMatch = [regex]::Match($taskSource, '(?s)(?:pub\s+)?const\s+' + [regex]::Escape($Name) + ':\s*&str\s*=\s*r(?<hash>#+)"(?<body>.*?)"\k<hash>;')
    if (!$taskMatch.Success) { throw "Missing Rust literal $Name" }
    $taskMatch.Groups['body'].Value.Replace("`r`n", "`n")
}
$taskCursor = Read-RowReadLiteral 'ROW_CURSOR'
$taskChunks = Read-RowReadLiteral 'CHUNKS'
$taskTests = Read-RowReadLiteral 'TEST_CONTENTS'
$taskSourceHash = (Get-FileHash -LiteralPath $taskSourcePath).Hash.ToLowerInvariant()
$taskDatabasePath = Join-Path $taskRoot 'app/build/generated/source/rustAndroid/main/com/ofairyo/gridtimer/data/AppStateDatabase.kt'
$taskDatabase = [IO.File]::ReadAllText($taskDatabasePath).Replace("`r`n", "`n")
foreach ($taskLiteral in @($taskCursor, $taskChunks)) {
    if (!$taskDatabase.Contains($taskLiteral.Trim())) { throw 'Generated snapshot reader differs from its Rust source' }
}
foreach ($taskBinding in @('database.querySnapshotRows(', 'readableDatabase.querySnapshotRows(', 'backup.querySnapshotRows(', '            querySnapshotRows(table, columns, rowSelection,')) {
    if (!$taskDatabase.Contains($taskBinding)) { throw "Production snapshot read binding is missing: $taskBinding" }
}
foreach ($taskUnsafe in @('database.query(', 'readableDatabase.query(', 'backup.query(', 'SELECT app_data_json FROM $HISTORY_TABLE')) {
    if ($taskDatabase.Contains($taskUnsafe)) { throw "Unbounded production snapshot read remains: $taskUnsafe" }
}
$taskCache = Join-Path $env:USERPROFILE '.gradle/caches/modules-2/files-2.1'
function Find-RowReadDependency([string]$Coordinate, [string]$Version, [string]$Name) {
    $taskFiles = @(Get-ChildItem -LiteralPath (Join-Path $taskCache "$Coordinate/$Version") -File -Recurse | Where-Object Name -eq $Name)
    if ($taskFiles.Count -ne 1) { throw "Missing dependency $Name" }
    $taskFiles[0].FullName
}
$taskCompiler = @(
    (Find-RowReadDependency 'org.jetbrains.kotlin/kotlin-compiler-embeddable' '1.9.24' 'kotlin-compiler-embeddable-1.9.24.jar'),
    (Find-RowReadDependency 'org.jetbrains.kotlin/kotlin-stdlib' '1.9.24' 'kotlin-stdlib-1.9.24.jar'),
    (Find-RowReadDependency 'org.jetbrains.kotlin/kotlin-script-runtime' '1.9.24' 'kotlin-script-runtime-1.9.24.jar'),
    (Find-RowReadDependency 'org.jetbrains.kotlin/kotlin-reflect' '1.6.10' 'kotlin-reflect-1.6.10.jar'),
    (Find-RowReadDependency 'org.jetbrains.kotlin/kotlin-daemon-embeddable' '1.9.24' 'kotlin-daemon-embeddable-1.9.24.jar'),
    (Find-RowReadDependency 'org.jetbrains.intellij.deps/trove4j' '1.0.20200330' 'trove4j-1.0.20200330.jar'),
    (Find-RowReadDependency 'org.jetbrains/annotations' '13.0' 'annotations-13.0.jar')
)
$taskRuntime = @(
    (Find-RowReadDependency 'org.jetbrains.kotlin/kotlin-stdlib' '1.9.24' 'kotlin-stdlib-1.9.24.jar'),
    (Find-RowReadDependency 'junit/junit' '4.13.2' 'junit-4.13.2.jar'),
    (Find-RowReadDependency 'org.hamcrest/hamcrest-core' '1.3' 'hamcrest-core-1.3.jar'),
    (Find-RowReadDependency 'org.jetbrains/annotations' '23.0.0' 'annotations-23.0.0.jar'),
    'C:\tools\android-sdk\platforms\android-35\android.jar'
)
$taskJava = 'C:\tools\java\jdk-17.0.18+8\bin\java.exe'
$taskEvidence = Join-Path $taskRoot "release_artifacts/verification/v$Version/snapshot_payload_mutation"
New-Item -ItemType Directory -Force -Path $taskEvidence | Out-Null
[IO.File]::WriteAllText((Join-Path $taskEvidence 'receipt.json'),
    ([ordered]@{passed=$false;status='running';sourceSha256=$taskSourceHash} | ConvertTo-Json),
    [Text.UTF8Encoding]::new($false))
function Invoke-RowReadJava([string[]]$Arguments, [string]$LogPath, [int]$TimeoutSeconds) {
    $taskStart = [Diagnostics.ProcessStartInfo]::new($taskJava)
    $taskStart.UseShellExecute = $false
    $taskStart.CreateNoWindow = $true
    $taskStart.RedirectStandardOutput = $true
    $taskStart.RedirectStandardError = $true
    foreach ($taskArgument in $Arguments) { $taskStart.ArgumentList.Add($taskArgument) }
    $taskProcess = [Diagnostics.Process]::new()
    $taskProcess.StartInfo = $taskStart
    try {
        if (!$taskProcess.Start()) { throw 'Cannot start isolated snapshot JVM' }
        $taskOut = $taskProcess.StandardOutput.ReadToEndAsync()
        $taskErr = $taskProcess.StandardError.ReadToEndAsync()
        if (!$taskProcess.WaitForExit($TimeoutSeconds * 1000)) {
            $taskProcess.Kill($true)
            $taskProcess.WaitForExit()
            [IO.File]::WriteAllText($LogPath, $taskOut.GetAwaiter().GetResult() + $taskErr.GetAwaiter().GetResult(), [Text.UTF8Encoding]::new($false))
            throw "Isolated snapshot JVM exceeded ${TimeoutSeconds}s: $LogPath"
        }
        [IO.File]::WriteAllText($LogPath, $taskOut.GetAwaiter().GetResult() + $taskErr.GetAwaiter().GetResult(), [Text.UTF8Encoding]::new($false))
        $taskProcess.ExitCode
    } finally { $taskProcess.Dispose() }
}
$taskResults = @()
foreach ($taskCase in @('baseline', 'cosmetic_only', 'removed_chunk_length_guard', 'removed_offset_progression')) {
    $taskDirectory = Join-Path $taskEvidence $taskCase
    New-Item -ItemType Directory -Force -Path $taskDirectory | Out-Null
    $taskPolicy = $taskChunks
    if ($taskCase -eq 'cosmetic_only') {
        $taskPolicy = $taskPolicy.Replace('Snapshot payload chunk did not match its declared byte length.', 'Snapshot byte count changed while reading.')
    }
    if ($taskCase -eq 'removed_chunk_length_guard') {
        $taskPolicy = $taskPolicy.Replace('        check(chunk.size == expected) { "Snapshot payload chunk did not match its declared byte length." }', '// Removed key integrity guard.')
    }
    if ($taskCase -eq 'removed_offset_progression') {
        $taskPolicy = $taskPolicy.Replace('        offset += expected', '// Removed offset progression.')
    }
    if ($taskCase -ne 'baseline' -and $taskPolicy -ceq $taskChunks) { throw "Mutation was not applied: $taskCase" }
    $taskProductionPath = Join-Path $taskDirectory 'SnapshotPayloadReader.kt'
    $taskTestPath = Join-Path $taskDirectory 'SnapshotPayloadChunksTest.kt'
    [IO.File]::WriteAllText($taskProductionPath, "package com.ofairyo.gridtimer.data`n" + $taskCursor + $taskPolicy, [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText($taskTestPath, $taskTests, [Text.UTF8Encoding]::new($false))
    $taskOutputPath = Join-Path $taskDirectory 'classes'
    $taskCompilerArguments = @('-Xmx512m','-cp',($taskCompiler -join ';'),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','17','-classpath',($taskRuntime -join ';'),'-d',$taskOutputPath,$taskProductionPath,$taskTestPath)
    $taskCompileExit = Invoke-RowReadJava $taskCompilerArguments (Join-Path $taskDirectory 'compile.log') 90
    if ($taskCompileExit -ne 0) { Get-Content (Join-Path $taskDirectory 'compile.log'); throw "Kotlin API compile failed: $taskCase" }
    $taskTestArguments = @('-Xmx256m', '-cp', ((@($taskOutputPath) + $taskRuntime) -join ';'), 'org.junit.runner.JUnitCore', 'com.ofairyo.gridtimer.data.SnapshotPayloadChunksTest')
    $taskTestExit = Invoke-RowReadJava $taskTestArguments (Join-Path $taskDirectory 'tests.log') 30
    $taskTestLog = [IO.File]::ReadAllText((Join-Path $taskDirectory 'tests.log'))
    $taskExpectedPass = $taskCase -in @('baseline', 'cosmetic_only')
    $taskMethod = switch ($taskCase) {
        'removed_chunk_length_guard' { 'shortOversizedAndMissingChunksCannotProduceASuccessfulPayload' }
        'removed_offset_progression' { 'largeUnicodeWorkspaceRoundTripsAcrossByteBoundaries' }
        default { '' }
    }
    $taskJUnitSuccess = $taskTestLog -match '(?m)^OK \(4 tests\)\s*$'
    $taskJUnitFailure = $taskTestLog -match '(?m)^FAILURES!!!\s*$' -and
        $taskTestLog -match '(?m)^Tests run: 4,\s+Failures: [1-9]\d*\s*$' -and
        $taskTestLog.Contains('java.lang.AssertionError')
    $taskBusinessFailure = $taskMethod -ne '' -and
        $taskTestLog.Contains($taskMethod + '(com.ofairyo.gridtimer.data.SnapshotPayloadChunksTest)')
    if ($taskCase -eq 'removed_chunk_length_guard') {
        $taskBusinessFailure = $taskBusinessFailure -and $taskTestLog.Contains('An incomplete or oversized payload must fail')
    }
    $taskPassed = if ($taskExpectedPass) {
        $taskTestExit -eq 0 -and $taskJUnitSuccess -and !$taskJUnitFailure
    } else {
        $taskTestExit -ne 0 -and $taskJUnitFailure -and $taskBusinessFailure -and !$taskJUnitSuccess
    }
    $taskResults += [ordered]@{name=$taskCase;passed=$taskPassed;testExit=$taskTestExit;expectedPass=$taskExpectedPass;junitTests=4;junitSuccess=$taskJUnitSuccess;assertionFailure=$taskJUnitFailure;businessMethod=$taskMethod;businessFailure=$taskBusinessFailure;sourceChanged=($taskPolicy -cne $taskChunks)}
    Write-Output "$taskCase passed=$taskPassed testExit=$taskTestExit"
    if (!$taskPassed) { Get-Content (Join-Path $taskDirectory 'tests.log'); throw "Unexpected mutation result: $taskCase" }
}
$taskUnchanged = $taskSourceHash -ceq (Get-FileHash -LiteralPath $taskSourcePath).Hash.ToLowerInvariant()
if (!$taskUnchanged) { throw 'Production Rust snapshot reader changed during verification' }
$taskReceipt = [ordered]@{passed=$true;sourceSha256=$taskSourceHash;productionUnchanged=$taskUnchanged;generatedDatabaseSha256=(Get-FileHash -LiteralPath $taskDatabasePath).Hash.ToLowerInvariant();tests=4;androidApiCompile=35;cases=$taskResults;deviceVerified=$false;evidence=$taskEvidence}
[IO.File]::WriteAllText((Join-Path $taskEvidence 'receipt.json'), ($taskReceipt | ConvertTo-Json -Depth 8), [Text.UTF8Encoding]::new($false))
$taskReceipt | ConvertTo-Json -Depth 8
