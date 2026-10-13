# v0.0.1 - Verify knowledge document transition access and stale-entry rejection.
param([string]$Version)
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if (!$Version) { $Version = [regex]::Match([IO.File]::ReadAllText((Join-Path $root 'app/build.gradle')), "versionName '([^']+)'").Groups[1].Value }
if ($Version -notmatch '^\d+\.\d+\.\d+\.\d+$') { throw 'Invalid Android version' }
$evidence = Join-Path $root "release_artifacts/verification/v$Version/document_transition_mutation"
$emitter = Join-Path $root 'native/gridtimer_native/src/sourcegen/android_document_transition.rs'
$actualTransition = Join-Path $root 'app/build/generated/source/rustAndroid/main/com/ofairyo/gridtimer/ui/KnowledgeDocumentTransition.kt'
$actualStudio = Join-Path $root 'app/build/generated/source/rustAndroid/main/com/ofairyo/gridtimer/ui/NoteStudioSheet.kt'
if (![IO.File]::Exists($actualTransition) -or ![IO.File]::Exists($actualStudio)) { throw 'Generate production Android sources before running this gate' }
$sourceHash = (Get-FileHash -LiteralPath $emitter).Hash
$generatedTransitionHash = (Get-FileHash -LiteralPath $actualTransition).Hash
$generatedStudioHash = (Get-FileHash -LiteralPath $actualStudio).Hash
$raw = [IO.File]::ReadAllText($emitter)

function Normalize-Lines([string]$Value) {
    $Value.Replace(([string][char]13 + [char]10), [string][char]10)
}

function Get-TransitionLiteral([string]$Source, [string]$Name) {
    $pattern = '(?s)(?:pub\s+)?const\s+' + $Name + ':\s*&str\s*=\s*r(?<hash>#+)"(?<body>.*?)"\k<hash>;'
    $match = [regex]::Match($Source, $pattern)
    if (!$match.Success) { throw "Missing production literal: $Name" }
    Normalize-Lines $match.Groups['body'].Value
}

function Get-TransitionDomain([string]$Source) {
    $start = $Source.IndexOf('internal fun knowledgeDocumentFrameIsCurrent(', [StringComparison]::Ordinal)
    $end = $Source.IndexOf('// A transparent input barrier', [StringComparison]::Ordinal)
    if ($start -lt 0 -or $end -le $start) { throw 'Production transition domain boundaries missing' }
    Normalize-Lines $Source.Substring($start, $end - $start).Trim()
}

function Replace-TransitionGuard([string]$Source, [string]$Before, [string]$After, [string]$Case) {
    if ([regex]::Matches($Source, [regex]::Escape($Before)).Count -ne 1) { throw "Missing or ambiguous production guard: $Case" }
    $Source.Replace($Before, $After)
}

$contents = Get-TransitionLiteral $raw 'CONTENTS'
$domain = Get-TransitionDomain $contents
$tests = Get-TransitionLiteral $raw 'TEST_CONTENTS'
$transitionEnd = Get-TransitionLiteral $raw 'TRANSITION_END'
$testCount = [regex]::Matches($tests, '@Test\s+fun ').Count
if ($testCount -lt 8) { throw 'Transition access boundary tests missing' }
$generatedContents = Normalize-Lines ([IO.File]::ReadAllText($actualTransition))
$studio = Normalize-Lines ([IO.File]::ReadAllText($actualStudio))
if ((Get-TransitionDomain $generatedContents) -cne $domain) { throw 'Generated production transition domain does not match its Rust emitter' }
if ([regex]::Matches($studio, [regex]::Escape($transitionEnd)).Count -ne 1) { throw 'Generated transition presentation does not match TRANSITION_END' }
foreach ($binding in @(
    'knowledgeDocumentFrameIsCurrent(',
    'KnowledgeDocumentTransitionAccess(',
    'frameAccess.mayDrawEditor()',
    'frameAccess.mayInteract()',
    'LocalDocumentTransitionActive provides interactiveFrame'
)) { if (!$studio.Contains($binding)) { throw "Production studio is not using transition access policy: $binding" } }

$cache = Join-Path $env:USERPROFILE '.gradle/caches/modules-2/files-2.1'
function Get-TransitionDependency([string]$Coordinate, [string]$DependencyVersion, [string]$Name) {
    $directory = Join-Path $cache ($Coordinate + '/' + $DependencyVersion)
    $files = @(Get-ChildItem -LiteralPath $directory -File -Recurse | Where-Object Name -eq $Name)
    if ($files.Count -ne 1) { throw "Installed transition verification dependency is missing or ambiguous: $Name" }
    $files[0].FullName
}
$compiler = @(
    (Get-TransitionDependency 'org.jetbrains.kotlin/kotlin-compiler-embeddable' '1.9.24' 'kotlin-compiler-embeddable-1.9.24.jar'),
    (Get-TransitionDependency 'org.jetbrains.kotlin/kotlin-stdlib' '1.9.24' 'kotlin-stdlib-1.9.24.jar'),
    (Get-TransitionDependency 'org.jetbrains.kotlin/kotlin-script-runtime' '1.9.24' 'kotlin-script-runtime-1.9.24.jar'),
    (Get-TransitionDependency 'org.jetbrains.kotlin/kotlin-reflect' '1.6.10' 'kotlin-reflect-1.6.10.jar'),
    (Get-TransitionDependency 'org.jetbrains.kotlin/kotlin-daemon-embeddable' '1.9.24' 'kotlin-daemon-embeddable-1.9.24.jar'),
    (Get-TransitionDependency 'org.jetbrains.intellij.deps/trove4j' '1.0.20200330' 'trove4j-1.0.20200330.jar'),
    (Get-TransitionDependency 'org.jetbrains/annotations' '13.0' 'annotations-13.0.jar')
)
$runtime = @(
    (Get-TransitionDependency 'org.jetbrains.kotlin/kotlin-stdlib' '1.9.24' 'kotlin-stdlib-1.9.24.jar'),
    (Get-TransitionDependency 'junit/junit' '4.13.2' 'junit-4.13.2.jar'),
    (Get-TransitionDependency 'org.hamcrest/hamcrest-core' '1.3' 'hamcrest-core-1.3.jar'),
    (Get-TransitionDependency 'org.jetbrains/annotations' '23.0.0' 'annotations-23.0.0.jar')
)
$java = 'C:\tools\java\jdk-17.0.18+8\bin\java.exe'
$utf8 = [Text.UTF8Encoding]::new($false)
$results = @()
$lineFeed = [string][char]10
foreach ($case in @('baseline', 'cosmetic_only', 'removed_workspace_guard', 'removed_entry_identity_guard', 'removed_current_interaction_guard', 'removed_encrypted_outgoing_guard')) {
    $directory = Join-Path $evidence $case
    New-Item -ItemType Directory -Force -Path $directory | Out-Null
    $caseDomain = $domain
    $cosmeticEmitterHash = $null
    $cosmeticStudioHash = $null
    if ($case -eq 'cosmetic_only') {
        $cosmeticEnd = Replace-TransitionGuard $transitionEnd 'fadeIn(animationSpec = tween(160))' 'fadeIn(animationSpec = tween(180))' $case
        $cosmeticEnd = Replace-TransitionGuard $cosmeticEnd 'fadeOut(animationSpec = tween(120))' 'fadeOut(animationSpec = tween(100))' $case
        # Modify the actual Rust presentation literal only in an evidence copy.
        $cosmeticEmitter = (Normalize-Lines $raw).Replace($transitionEnd, $cosmeticEnd)
        if ((Get-TransitionLiteral $cosmeticEmitter 'TRANSITION_END') -cne $cosmeticEnd) { throw 'Cosmetic mutation did not change the actual presentation literal' }
        $caseDomain = Get-TransitionDomain (Get-TransitionLiteral $cosmeticEmitter 'CONTENTS')
        if ($caseDomain -cne $domain) { throw 'Cosmetic mutation changed transition authorization logic' }
        $cosmeticStudio = $studio.Replace($transitionEnd, $cosmeticEnd)
        if ($cosmeticStudio -ceq $studio) { throw 'Cosmetic mutation did not change generated presentation' }
        $cosmeticEmitterPath = Join-Path $directory 'android_document_transition_style.rs'
        $cosmeticStudioPath = Join-Path $directory 'NoteStudioSheet_style.kt'
        [IO.File]::WriteAllText($cosmeticEmitterPath, $cosmeticEmitter, $utf8)
        [IO.File]::WriteAllText($cosmeticStudioPath, $cosmeticStudio, $utf8)
        $cosmeticEmitterHash = (Get-FileHash -LiteralPath $cosmeticEmitterPath).Hash.ToLowerInvariant()
        $cosmeticStudioHash = (Get-FileHash -LiteralPath $cosmeticStudioPath).Hash.ToLowerInvariant()
    }
    switch ($case) {
        'removed_workspace_guard' {
            $caseDomain = Replace-TransitionGuard $caseDomain ': Boolean = workspaceMatches && frameNoteId == selectedNoteId &&' ': Boolean = frameNoteId == selectedNoteId &&' $case
            $caseDomain = Replace-TransitionGuard $caseDomain 'if (!workspaceMatches || recordDeleted) return false' 'if (recordDeleted) return false' $case
        }
        'removed_entry_identity_guard' {
            $caseDomain = Replace-TransitionGuard $caseDomain 'frameEntryKey == currentEntryKey' 'true' $case
        }
        'removed_current_interaction_guard' {
            $caseDomain = Replace-TransitionGuard $caseDomain 'fun mayInteract(): Boolean = currentEntry && mayDrawEditor()' 'fun mayInteract(): Boolean = mayDrawEditor()' $case
        }
        'removed_encrypted_outgoing_guard' {
            $caseDomain = Replace-TransitionGuard $caseDomain 'return !snapshotProtected && recordPresent && !recordProtected' 'return recordPresent' $case
        }
    }
    $production = 'package com.ofairyo.gridtimer.ui' + $lineFeed + $lineFeed + $caseDomain + $lineFeed
    $helperFile = Join-Path $directory 'KnowledgeDocumentTransitionDomain.kt'
    $testFile = Join-Path $directory 'KnowledgeDocumentTransitionTest.kt'
    [IO.File]::WriteAllText($helperFile, $production, $utf8)
    [IO.File]::WriteAllText($testFile, $tests, $utf8)
    $classes = Join-Path $directory ('classes_' + [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds())
    & $java -Xmx512m -cp ($compiler -join ';') org.jetbrains.kotlin.cli.jvm.K2JVMCompiler -no-stdlib -no-reflect -jvm-target 17 -classpath ($runtime -join ';') -d $classes $helperFile $testFile *> (Join-Path $directory 'compile.log')
    if ($LASTEXITCODE -ne 0) { Get-Content -LiteralPath (Join-Path $directory 'compile.log') -Tail 20; throw "Mutation compilation failed: $case" }
    & $java -Xmx256m -cp ($classes + ';' + ($runtime -join ';')) org.junit.runner.JUnitCore com.ofairyo.gridtimer.ui.KnowledgeDocumentTransitionTest *> (Join-Path $directory 'tests.log')
    $exitCode = $LASTEXITCODE
    $log = [IO.File]::ReadAllText((Join-Path $directory 'tests.log'))
    $expectedPass = $case -in @('baseline', 'cosmetic_only')
    $requiredFailure = switch ($case) {
        'removed_workspace_guard' { 'workspaceChangeRevokesBothActiveAndOutgoingEditors' }
        'removed_entry_identity_guard' { 'oldCallbackCannotCloseAReopenedRecordOrAnotherWorkspace' }
        'removed_current_interaction_guard' { 'ordinaryExitMayDrawButCannotInteract' }
        'removed_encrypted_outgoing_guard' { 'protectedEditorRequiresItsCurrentSelection' }
        default { '' }
    }
    $passed = if ($expectedPass) { $exitCode -eq 0 -and $log.Contains("OK ($testCount tests)") } else { $exitCode -ne 0 -and $log.Contains('java.lang.AssertionError') -and $log.Contains($requiredFailure) }
    $results += [ordered]@{
        case = $case
        passed = $passed
        expectedPass = $expectedPass
        compileSucceeded = $true
        exitCode = $exitCode
        tests = $testCount
        requiredFailure = $requiredFailure
        helperSha256 = (Get-FileHash -LiteralPath $helperFile).Hash.ToLowerInvariant()
        testsSha256 = (Get-FileHash -LiteralPath $testFile).Hash.ToLowerInvariant()
        uiStyleChanged = ($case -eq 'cosmetic_only')
        cosmeticEmitterSha256 = $cosmeticEmitterHash
        cosmeticStudioSha256 = $cosmeticStudioHash
    }
    if (!$passed) { Get-Content -LiteralPath (Join-Path $directory 'tests.log') -Tail 25; throw "Unexpected mutation result: $case" }
    Write-Output "$case verified"
}
if ((Get-FileHash -LiteralPath $emitter).Hash -cne $sourceHash -or
    (Get-FileHash -LiteralPath $actualTransition).Hash -cne $generatedTransitionHash -or
    (Get-FileHash -LiteralPath $actualStudio).Hash -cne $generatedStudioHash) { throw 'Production changed during transition mutation verification' }
[ordered]@{
    passed = $true
    version = $Version
    tests = $testCount
    productionUnchanged = $true
    sourceSha256 = $sourceHash.ToLowerInvariant()
    generatedTransitionSha256 = $generatedTransitionHash.ToLowerInvariant()
    generatedStudioSha256 = $generatedStudioHash.ToLowerInvariant()
    helperLiteralSha256 = (Get-FileHash -LiteralPath (Join-Path $evidence 'baseline/KnowledgeDocumentTransitionDomain.kt')).Hash.ToLowerInvariant()
    testsSha256 = (Get-FileHash -LiteralPath (Join-Path $evidence 'baseline/KnowledgeDocumentTransitionTest.kt')).Hash.ToLowerInvariant()
    productionIntegrationVerified = $true
    cases = $results
    hostOnly = $true
    deviceVerified = $false
} | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $evidence 'receipt.json') -Encoding utf8
