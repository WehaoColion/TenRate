# v0.0.1 - Verify knowledge hierarchy boundaries from the Rust generated Android helper.
param([string]$Version, [switch]$SkipGeneratedCheck)
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if (!$Version) { $Version = [regex]::Match([IO.File]::ReadAllText((Join-Path $root 'app/build.gradle')), "versionName '([^']+)'").Groups[1].Value }
if ($Version -notmatch '^\d+\.\d+\.\d+\.\d+$') { throw 'Invalid Android version' }
$emitter = Join-Path $root 'native/gridtimer_native/src/sourcegen/android_knowledge_hierarchy.rs'
$generated = Join-Path $root 'app/build/generated/source/rustAndroid/main/com/ofairyo/gridtimer/data/KnowledgeHierarchy.kt'
$classes = Join-Path $root 'app/build/tmp/kotlin-classes/release'
$evidence = Join-Path $root "release_artifacts/verification/v$Version/knowledge_hierarchy_mutation"
if (![IO.File]::Exists((Join-Path $classes 'com/ofairyo/gridtimer/data/NoteEntry.class'))) { throw 'Compile release Android model classes before running this gate' }
$originalHash = (Get-FileHash -LiteralPath $emitter).Hash
$raw = [IO.File]::ReadAllText($emitter)
function Normalize-HierarchyLines([string]$Value) { $Value.Replace(([string][char]13 + [char]10), [string][char]10) }
function Read-HierarchyLiteral([string]$Source, [string]$Name) {
    $match = [regex]::Match($Source, '(?s)pub const ' + $Name + ':\s*&str\s*=\s*r(?<hash>#+)"(?<body>.*?)"\k<hash>;')
    if (!$match.Success) { throw "Missing Rust source literal: $Name" }
    Normalize-HierarchyLines $match.Groups['body'].Value
}
function Replace-HierarchyMutation([string]$Source, [string]$Before, [string]$After, [string]$Name) {
    if ([regex]::Matches($Source, [regex]::Escape($Before)).Count -ne 1) { throw "Missing or repeated hierarchy mutation anchor: $Name" }
    $Source.Replace($Before, $After)
}
$helper = Read-HierarchyLiteral $raw 'CONTENTS'
$tests = Read-HierarchyLiteral $raw 'TEST_CONTENTS'
$testCount = [regex]::Matches($tests, '@Test\s+fun ').Count
$generatedHash = $null
if (!$SkipGeneratedCheck) {
    if (![IO.File]::Exists($generated)) { throw 'Generate production Android sources before running this gate' }
    if ((Normalize-HierarchyLines ([IO.File]::ReadAllText($generated))) -cne $helper) { throw 'Generated production hierarchy does not match its Rust source literal' }
    $generatedHash = (Get-FileHash -LiteralPath $generated).Hash
}
$cache = Join-Path $env:USERPROFILE '.gradle/caches/modules-2/files-2.1'
function Find-HierarchyDependency([string]$Coordinate, [string]$DependencyVersion, [string]$Name) {
    $directory = Join-Path $cache ($Coordinate + '/' + $DependencyVersion)
    $found = @(Get-ChildItem -LiteralPath $directory -File -Recurse | Where-Object Name -eq $Name)
    if ($found.Count -ne 1) { throw "Missing or ambiguous installed dependency: $Name" }
    $found[0].FullName
}
$compiler = @(
    (Find-HierarchyDependency 'org.jetbrains.kotlin/kotlin-compiler-embeddable' '1.9.24' 'kotlin-compiler-embeddable-1.9.24.jar'),
    (Find-HierarchyDependency 'org.jetbrains.kotlin/kotlin-stdlib' '1.9.24' 'kotlin-stdlib-1.9.24.jar'),
    (Find-HierarchyDependency 'org.jetbrains.kotlin/kotlin-script-runtime' '1.9.24' 'kotlin-script-runtime-1.9.24.jar'),
    (Find-HierarchyDependency 'org.jetbrains.kotlin/kotlin-reflect' '1.6.10' 'kotlin-reflect-1.6.10.jar'),
    (Find-HierarchyDependency 'org.jetbrains.kotlin/kotlin-daemon-embeddable' '1.9.24' 'kotlin-daemon-embeddable-1.9.24.jar'),
    (Find-HierarchyDependency 'org.jetbrains.intellij.deps/trove4j' '1.0.20200330' 'trove4j-1.0.20200330.jar'),
    (Find-HierarchyDependency 'org.jetbrains/annotations' '13.0' 'annotations-13.0.jar')
)
$runtime = @(
    (Find-HierarchyDependency 'org.jetbrains.kotlin/kotlin-stdlib' '1.9.24' 'kotlin-stdlib-1.9.24.jar'),
    (Find-HierarchyDependency 'junit/junit' '4.13.2' 'junit-4.13.2.jar'),
    (Find-HierarchyDependency 'org.hamcrest/hamcrest-core' '1.3' 'hamcrest-core-1.3.jar'),
    (Find-HierarchyDependency 'org.jetbrains/annotations' '23.0.0' 'annotations-23.0.0.jar'),
    (Find-HierarchyDependency 'org.jetbrains.kotlinx/kotlinx-serialization-json-jvm' '1.6.3' 'kotlinx-serialization-json-jvm-1.6.3.jar'),
    (Find-HierarchyDependency 'org.jetbrains.kotlinx/kotlinx-serialization-core-jvm' '1.6.3' 'kotlinx-serialization-core-jvm-1.6.3.jar')
)
$java = 'C:\tools\java\jdk-17.0.18+8\bin\java.exe'
$utf8 = [Text.UTF8Encoding]::new($false)
$results = @()
foreach ($case in @('baseline', 'cosmetic_only', 'removed_descendant_guard', 'removed_protected_parent_guard', 'removed_inaccessible_parent_fallback')) {
    $directory = Join-Path $evidence $case
    New-Item -ItemType Directory -Force -Path $directory | Out-Null
    $candidate = $helper
    switch ($case) {
        'cosmetic_only' { $candidate = Replace-HierarchyMutation $candidate 'joinToString(" / ")' 'joinToString(" > ")' $case }
        'removed_descendant_guard' { $candidate = Replace-HierarchyMutation $candidate 'val forbidden = descendantIds(id) + id' 'val forbidden = setOf(id)' $case }
        'removed_protected_parent_guard' { $candidate = Replace-HierarchyMutation $candidate 'return nodes.values.filter { mayCreateInside(it.id) && it.id !in forbidden }' 'return nodes.values.filter { it.id !in forbidden }' $case }
        'removed_inaccessible_parent_fallback' { $candidate = Replace-HierarchyMutation $candidate 'source.getValue(parent).note?.encryption != null ->' 'false ->' $case }
    }
    $helperPath = Join-Path $directory 'KnowledgeHierarchy.kt'
    $testsPath = Join-Path $directory 'KnowledgeHierarchyTest.kt'
    [IO.File]::WriteAllText($helperPath, $candidate, $utf8)
    [IO.File]::WriteAllText($testsPath, $tests, $utf8)
    $caseClasses = Join-Path $directory ('classes_' + [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds())
    & $java -Xmx512m -cp ($compiler -join ';') org.jetbrains.kotlin.cli.jvm.K2JVMCompiler -no-stdlib -no-reflect -jvm-target 17 -classpath ($classes + ';' + ($runtime -join ';')) -d $caseClasses $helperPath $testsPath *> (Join-Path $directory 'compile.log')
    if ($LASTEXITCODE -ne 0) { Get-Content -LiteralPath (Join-Path $directory 'compile.log') -Tail 30; throw "Hierarchy mutation compilation failed: $case" }
    & $java -Xmx256m -cp ($caseClasses + ';' + $classes + ';' + ($runtime -join ';')) org.junit.runner.JUnitCore com.ofairyo.gridtimer.data.KnowledgeHierarchyTest *> (Join-Path $directory 'tests.log')
    $exitCode = $LASTEXITCODE
    $log = [IO.File]::ReadAllText((Join-Path $directory 'tests.log'))
    $expectedPass = $case -in @('baseline', 'cosmetic_only')
    $requiredFailure = switch ($case) {
        'removed_descendant_guard' { 'moveTargetsRejectSelfDescendantsAndProtectedParents' }
        'removed_protected_parent_guard' { 'moveTargetsRejectSelfDescendantsAndProtectedParents' }
        'removed_inaccessible_parent_fallback' { 'deletedMissingOrEncryptedParentCannotHideChildren' }
        default { '' }
    }
    $passed = if ($expectedPass) { $exitCode -eq 0 -and $log.Contains("OK ($testCount tests)") } else { $exitCode -ne 0 -and $log.Contains('java.lang.AssertionError') -and $log.Contains($requiredFailure) }
    $results += [ordered]@{ case=$case; passed=$passed; expectedPass=$expectedPass; exitCode=$exitCode; tests=$testCount; requiredFailure=$requiredFailure; helperSha256=(Get-FileHash -LiteralPath $helperPath).Hash.ToLowerInvariant(); cosmeticChanged=($case -eq 'cosmetic_only') }
    if (!$passed) { Get-Content -LiteralPath (Join-Path $directory 'tests.log') -Tail 30; throw "Unexpected hierarchy mutation result: $case" }
    Write-Output "$case verified"
}
if ((Get-FileHash -LiteralPath $emitter).Hash -cne $originalHash) { throw 'Rust production source changed during hierarchy verification' }
if ($generatedHash -and (Get-FileHash -LiteralPath $generated).Hash -cne $generatedHash) { throw 'Generated production helper changed during hierarchy verification' }
[ordered]@{ passed=$true; version=$Version; tests=$testCount; productionUnchanged=$true; generatedChecked=(!$SkipGeneratedCheck); modelClasses='actual release compiled models'; sourceSha256=$originalHash.ToLowerInvariant(); generatedSha256=$generatedHash; cases=$results; hostOnly=$true; deviceVerified=$false } | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $evidence 'receipt.json') -Encoding utf8
