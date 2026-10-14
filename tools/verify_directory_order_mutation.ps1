# v0.0.1 - Execute directory order domain mutations without changing production files.
param([string]$Version, [switch]$SkipGeneratedCheck)
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if (!$Version) { $Version = [regex]::Match([IO.File]::ReadAllText((Join-Path $root 'app/build.gradle')), "versionName '([^']+)'").Groups[1].Value }
if ($Version -notmatch '^\d+\.\d+\.\d+\.\d+$') { throw 'Invalid Android version' }
$emitter = Join-Path $root 'native/gridtimer_native/src/sourcegen/android_knowledge_directory_order.rs'
$generated = Join-Path $root 'app/build/generated/source/rustAndroid/main/com/ofairyo/gridtimer/data/KnowledgeDirectoryOrder.kt'
$classes = Join-Path $root 'app/build/tmp/kotlin-classes/release'
$evidence = Join-Path $root "release_artifacts/verification/v$Version/directory_order_mutation"
if (![IO.File]::Exists((Join-Path $classes 'com/ofairyo/gridtimer/data/NoteEntry.class'))) { throw 'Compile release Android model classes before running this gate' }
$originalHash = (Get-FileHash -LiteralPath $emitter).Hash
$raw = [IO.File]::ReadAllText($emitter)
function Normalize-DirectoryOrderLines([string]$Value) { $Value.Replace(([string][char]13 + [char]10), [string][char]10) }
function Read-DirectoryOrderLiteral([string]$Source, [string]$Name) {
    $match = [regex]::Match($Source, '(?s)pub const ' + $Name + ':\s*&str\s*=\s*r(?<hash>#+)"(?<body>.*?)"\k<hash>;')
    if (!$match.Success) { throw "Missing Rust source literal: $Name" }
    Normalize-DirectoryOrderLines $match.Groups['body'].Value
}
function Replace-DirectoryOrderMutation([string]$Source, [string]$Before, [string]$After, [string]$Name) {
    if ([regex]::Matches($Source, [regex]::Escape($Before)).Count -ne 1) { throw "Missing or repeated directory order mutation anchor: $Name" }
    $Source.Replace($Before, $After)
}
$helper = Read-DirectoryOrderLiteral $raw 'CONTENTS'
$tests = Read-DirectoryOrderLiteral $raw 'TEST_CONTENTS'
$testCount = [regex]::Matches($tests, '@Test\s+fun ').Count
$generatedHash = $null
if (!$SkipGeneratedCheck) {
    if (![IO.File]::Exists($generated)) { throw 'Generate production Android sources before running this gate' }
    if ((Normalize-DirectoryOrderLines ([IO.File]::ReadAllText($generated))) -cne $helper) { throw 'Generated production directory order does not match its Rust source literal' }
    $generatedHash = (Get-FileHash -LiteralPath $generated).Hash
}
$cache = Join-Path $env:USERPROFILE '.gradle/caches/modules-2/files-2.1'
function Find-DirectoryOrderDependency([string]$Coordinate, [string]$DependencyVersion, [string]$Name) {
    $directory = Join-Path $cache ($Coordinate + '/' + $DependencyVersion)
    $found = @(Get-ChildItem -LiteralPath $directory -File -Recurse | Where-Object Name -eq $Name)
    if ($found.Count -ne 1) { throw "Missing or ambiguous installed dependency: $Name" }
    $found[0].FullName
}
$compiler = @(
    (Find-DirectoryOrderDependency 'org.jetbrains.kotlin/kotlin-compiler-embeddable' '1.9.24' 'kotlin-compiler-embeddable-1.9.24.jar'),
    (Find-DirectoryOrderDependency 'org.jetbrains.kotlin/kotlin-stdlib' '1.9.24' 'kotlin-stdlib-1.9.24.jar'),
    (Find-DirectoryOrderDependency 'org.jetbrains.kotlin/kotlin-script-runtime' '1.9.24' 'kotlin-script-runtime-1.9.24.jar'),
    (Find-DirectoryOrderDependency 'org.jetbrains.kotlin/kotlin-reflect' '1.6.10' 'kotlin-reflect-1.6.10.jar'),
    (Find-DirectoryOrderDependency 'org.jetbrains.kotlin/kotlin-daemon-embeddable' '1.9.24' 'kotlin-daemon-embeddable-1.9.24.jar'),
    (Find-DirectoryOrderDependency 'org.jetbrains.intellij.deps/trove4j' '1.0.20200330' 'trove4j-1.0.20200330.jar'),
    (Find-DirectoryOrderDependency 'org.jetbrains/annotations' '13.0' 'annotations-13.0.jar')
)
$runtime = @(
    (Find-DirectoryOrderDependency 'org.jetbrains.kotlin/kotlin-stdlib' '1.9.24' 'kotlin-stdlib-1.9.24.jar'),
    (Find-DirectoryOrderDependency 'junit/junit' '4.13.2' 'junit-4.13.2.jar'),
    (Find-DirectoryOrderDependency 'org.hamcrest/hamcrest-core' '1.3' 'hamcrest-core-1.3.jar'),
    (Find-DirectoryOrderDependency 'org.jetbrains/annotations' '23.0.0' 'annotations-23.0.0.jar'),
    (Find-DirectoryOrderDependency 'org.jetbrains.kotlinx/kotlinx-serialization-json-jvm' '1.6.3' 'kotlinx-serialization-json-jvm-1.6.3.jar'),
    (Find-DirectoryOrderDependency 'org.jetbrains.kotlinx/kotlinx-serialization-core-jvm' '1.6.3' 'kotlinx-serialization-core-jvm-1.6.3.jar'),
    (Find-DirectoryOrderDependency 'org.jetbrains.kotlinx/kotlinx-coroutines-core-jvm' '1.8.1' 'kotlinx-coroutines-core-jvm-1.8.1.jar'),
    'C:\tools\android-sdk\platforms\android-35\android.jar'
)
$java = 'C:\tools\java\jdk-17.0.18+8\bin\java.exe'
$utf8 = [Text.UTF8Encoding]::new($false)
$results = @()
foreach ($case in @('baseline', 'cosmetic_only', 'removed_workspace_guard', 'cancelled_as_release', 'ignored_durable_failure', 'removed_collection_frame_guard')) {
    $directory = Join-Path $evidence $case
    New-Item -ItemType Directory -Force -Path $directory | Out-Null
    $candidate = $helper
    switch ($case) {
        'removed_collection_frame_guard' { $candidate = Replace-DirectoryOrderMutation $candidate 'currentFrame && collectionFrame && visible && !busy' 'collectionFrame && visible && !busy' $case }
        'cosmetic_only' { $candidate = Replace-DirectoryOrderMutation $candidate 'KNOWLEDGE_DIRECTORY_DRAG_LABEL = "长按拖动排序"' 'KNOWLEDGE_DIRECTORY_DRAG_LABEL = "拖动调整顺序"' $case }
        'removed_workspace_guard' { $candidate = Replace-DirectoryOrderMutation $candidate 'workspaceKey == other.workspaceKey && parentId == other.parentId &&' 'true && parentId == other.parentId &&' $case }
        'cancelled_as_release' {
            $before = @'
    fun cancel() {
        isActive = false
        pendingIds = initialIds
    }
'@
            $candidate = Replace-DirectoryOrderMutation $candidate (Normalize-DirectoryOrderLines $before) '    fun cancel() { }' $case
        }
        'ignored_durable_failure' { $candidate = Replace-DirectoryOrderMutation $candidate 'return if (write(proposed)) proposed else null' 'write(proposed); return proposed' $case }
    }
    $helperPath = Join-Path $directory 'KnowledgeDirectoryOrder.kt'
    $testsPath = Join-Path $directory 'KnowledgeDirectoryOrderTest.kt'
    [IO.File]::WriteAllText($helperPath, $candidate, $utf8)
    [IO.File]::WriteAllText($testsPath, $tests, $utf8)
    $caseClasses = Join-Path $directory ('classes_' + [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds())
    & $java -Xmx512m -cp ($compiler -join ';') org.jetbrains.kotlin.cli.jvm.K2JVMCompiler -no-stdlib -no-reflect -jvm-target 17 ("-Xfriend-paths=" + $classes) -classpath ($classes + ';' + ($runtime -join ';')) -d $caseClasses $helperPath $testsPath *> (Join-Path $directory 'compile.log')
    if ($LASTEXITCODE -ne 0) { Get-Content -LiteralPath (Join-Path $directory 'compile.log') -Tail 30; throw "DirectoryOrder mutation compilation failed: $case" }
    & $java -Xmx256m -cp ($caseClasses + ';' + $classes + ';' + ($runtime -join ';')) org.junit.runner.JUnitCore com.ofairyo.gridtimer.data.KnowledgeDirectoryOrderTest *> (Join-Path $directory 'tests.log')
    $exitCode = $LASTEXITCODE
    $log = [IO.File]::ReadAllText((Join-Path $directory 'tests.log'))
    $expectedPass = $case -in @('baseline', 'cosmetic_only')
    $requiredFailure = switch ($case) {
        'removed_collection_frame_guard' { 'collectionInteractionAllowsRecordlessDirectoryAndRejectsStaleHiddenBusyOrEditorFrames' }
        'removed_workspace_guard' { 'staleWorkspaceParentPathMembershipAndPermissionsRejectReleaseAndWrite' }
        'cancelled_as_release' { 'cancelAfterMovingNeverReturnsACommitAndRestoresThePreview' }
        'ignored_durable_failure' { 'failedDurableWriteNeverPublishesAndInvalidOrdersNeverReachTheWriter' }
        default { '' }
    }
    $passed = if ($expectedPass) { $exitCode -eq 0 -and $log.Contains("OK ($testCount tests)") } else { $exitCode -ne 0 -and ($log.Contains('java.lang.AssertionError') -or $log.Contains('org.junit.ComparisonFailure')) -and $log.Contains($requiredFailure) }
    $results += [ordered]@{ case=$case; passed=$passed; expectedPass=$expectedPass; exitCode=$exitCode; tests=$testCount; requiredFailure=$requiredFailure; helperSha256=(Get-FileHash -LiteralPath $helperPath).Hash.ToLowerInvariant(); cosmeticChanged=($case -eq 'cosmetic_only') }
    if (!$passed) { Get-Content -LiteralPath (Join-Path $directory 'tests.log') -Tail 30; throw "Unexpected directory order mutation result: $case" }
    Write-Output "$case verified"
}
if ((Get-FileHash -LiteralPath $emitter).Hash -cne $originalHash) { throw 'Rust production source changed during directory order verification' }
if ($generatedHash -and (Get-FileHash -LiteralPath $generated).Hash -cne $generatedHash) { throw 'Generated production helper changed during directory order verification' }
[ordered]@{ passed=$true; version=$Version; tests=$testCount; productionUnchanged=$true; generatedChecked=(!$SkipGeneratedCheck); modelClasses='actual release compiled models'; sourceSha256=$originalHash.ToLowerInvariant(); generatedSha256=$generatedHash; cases=$results; hostOnly=$true; deviceVerified=$false } | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $evidence 'receipt.json') -Encoding utf8
