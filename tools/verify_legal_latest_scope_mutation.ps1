# v0.0.1 - Exercise current-content legal scope in an isolated crate copy.
[CmdletBinding()]
param([string]$Version = '')
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
$scopeRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$scopeCrate = Join-Path $scopeRoot 'native/gridtimer_native'
$scopeGradle = Join-Path $scopeRoot 'app/build.gradle'
if ([string]::IsNullOrWhiteSpace($Version)) {
    $Version = [regex]::Match([IO.File]::ReadAllText($scopeGradle), "versionName '([^']+)'").Groups[1].Value
}
if ($Version -notmatch '^\d+\.\d+\.\d+\.\d+$') { throw 'Invalid legal scope evidence version' }
$scopeRunId = [Guid]::NewGuid().ToString('N')
$scopeEvidence = Join-Path $scopeRoot "release_artifacts/verification/v$Version/legal_latest_scope_mutation/$scopeRunId"
if (Test-Path -LiteralPath $scopeEvidence) { throw 'Refusing to overwrite an existing mutation run' }
[IO.Directory]::CreateDirectory($scopeEvidence) | Out-Null
$scopeCopyCrate = Join-Path $scopeEvidence 'native/gridtimer_native'
[IO.Directory]::CreateDirectory($scopeCopyCrate) | Out-Null
$scopeRelativeSource = 'native/gridtimer_native/src/legal_scan.rs'
$scopeSource = Join-Path $scopeRoot $scopeRelativeSource
$scopeTestSource = Join-Path $scopeCrate 'src/legal_scan_current_tests.rs'
$scopeOriginalBytes = [IO.File]::ReadAllBytes($scopeSource)
$scopeOriginal = [Text.Encoding]::UTF8.GetString($scopeOriginalBytes).Replace("`r`n", "`n")
$scopeSourceHash = (Get-FileHash -LiteralPath $scopeSource -Algorithm SHA256).Hash.ToLowerInvariant()
$scopeTestsHash = (Get-FileHash -LiteralPath $scopeTestSource -Algorithm SHA256).Hash.ToLowerInvariant()
$scopeVerifierHash = (Get-FileHash -LiteralPath $PSCommandPath -Algorithm SHA256).Hash.ToLowerInvariant()
$scopeTestNames = @(([regex]'(?m)#\[test\]\s*fn\s+(?<name>latest_scope_[A-Za-z0-9_]+)\s*\(').Matches(
    [IO.File]::ReadAllText($scopeTestSource)) | ForEach-Object { $_.Groups['name'].Value })
if ($scopeTestNames.Count -ne 6) { throw 'Expected six current-content domain specifications' }
$scopeTextFailure = 'latest_scope_uses_current_page_and_sticky_content_once_even_when_history_is_newer'
$scopeAttachmentFailure = 'latest_scope_current_attachments_are_deduplicated_and_history_files_do_not_reduce_coverage'
if ($scopeTextFailure -notin $scopeTestNames -or $scopeAttachmentFailure -notin $scopeTestNames) {
    throw 'Required domain failure specifications are missing'
}

function Replace-ScopeOnce([string]$Text, [string]$Before, [string]$After) {
    if ([regex]::Matches($Text, [regex]::Escape($Before)).Count -ne 1) {
        throw 'Mutation anchor is missing or ambiguous'
    }
    return $Text.Replace($Before, $After)
}

$scopeCosmetic = Replace-ScopeOnce $scopeOriginal `
    'field_lines(note, &[("title", "标题"), ("content", "正文")])' `
    'field_lines(note, &[("title", "页面标题"), ("content", "页面正文")])'
$scopeCurrentFunction = [regex]::Match($scopeOriginal, '(?ms)^fn add_current_note\([^\n]*\{.*?^\}').Value
if (!$scopeCurrentFunction) { throw 'Current-note collector function is missing' }
$scopeHistoryInsertion = @'

    for key in ["revisions", "versions"] {
        for snapshot in array(note, key) {
            add_current_note(c, snapshot, note_id);
        }
    }
'@
$scopeHistoryFunction = $scopeCurrentFunction.Substring(0, $scopeCurrentFunction.Length - 1) + $scopeHistoryInsertion + "`n}"
$scopeHistory = Replace-ScopeOnce $scopeOriginal $scopeCurrentFunction $scopeHistoryFunction
$scopeCatalog = Replace-ScopeOnce $scopeOriginal '    if explicit_document {' '    if false && explicit_document {'
$scopeVariants = @(
    @{name='baseline';source=$scopeOriginal;expectFailure=$false;requiredFailure=''},
    @{name='cosmetic';source=$scopeCosmetic;expectFailure=$false;requiredFailure=''},
    @{name='history_included';source=$scopeHistory;expectFailure=$true;requiredFailure=$scopeTextFailure},
    @{name='history_attachment_catalog';source=$scopeCatalog;expectFailure=$true;requiredFailure=$scopeAttachmentFailure}
)

# Enumerate only required crate inputs. Never copy target, dependencies or caches.
$scopeInputs = @()
foreach ($name in @('Cargo.toml', 'Cargo.lock', 'build.rs')) {
    $path = Join-Path $scopeCrate $name
    if ($name -ne 'build.rs' -and !(Test-Path -LiteralPath $path -PathType Leaf)) { throw "Missing crate input: $name" }
    if (Test-Path -LiteralPath $path -PathType Leaf) { $scopeInputs += Get-Item -LiteralPath $path }
}
foreach ($name in @('src', 'assets', 'examples')) {
    $directory = Join-Path $scopeCrate $name
    if (!(Test-Path -LiteralPath $directory -PathType Container)) {
        if ($name -eq 'src') { throw 'Crate source directory is missing' }
        continue
    }
    $entries = @(Get-ChildItem -LiteralPath $directory -Recurse -Force)
    if (@($entries | Where-Object { $_.Attributes -band [IO.FileAttributes]::ReparsePoint }).Count -ne 0) {
        throw "Refusing to copy linked crate inputs: $name"
    }
    $scopeInputs += $entries | Where-Object { !$_.PSIsContainer }
}
# product_identity.rs includes this file at compile time, even with a test filter.
$scopeInputs += Get-Item -LiteralPath $scopeGradle
$scopeInputRecords = @()
foreach ($inputFile in $scopeInputs) {
    $relative = [IO.Path]::GetRelativePath($scopeRoot, $inputFile.FullName)
    $destination = [IO.Path]::GetFullPath((Join-Path $scopeEvidence $relative))
    if (!$destination.StartsWith($scopeEvidence + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Isolated input path escaped its run directory'
    }
    if (Test-Path -LiteralPath $destination) { throw "Refusing to overwrite isolated input: $relative" }
    [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($destination)) | Out-Null
    $before = (Get-FileHash -LiteralPath $inputFile.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    Copy-Item -LiteralPath $inputFile.FullName -Destination $destination
    if ((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash.ToLowerInvariant() -cne $before) {
        throw "Isolated input copy verification failed: $relative"
    }
    $scopeInputRecords += [ordered]@{path=$relative.Replace('\','/');sha256=$before}
}

$scopeEnvironmentNames = @('RUSTUP_TOOLCHAIN', 'LIB', 'CARGO_HOME', 'CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER', 'CARGO_TARGET_DIR', 'CARGO_INCREMENTAL')
$scopeOldEnvironment = @{}
foreach ($name in $scopeEnvironmentNames) { $scopeOldEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
$scopeResults = @()
$scopePassed = $false
$scopeProductionUnchanged = $false
try {
    $env:RUSTUP_TOOLCHAIN = 'stable-x86_64-pc-windows-msvc'
    $scopeRustc = (& rustup which --toolchain $env:RUSTUP_TOOLCHAIN rustc).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'Windows Rust toolchain is unavailable' }
    $scopeToolchain = Split-Path -Parent (Split-Path -Parent $scopeRustc)
    $scopeCargo = Join-Path $scopeToolchain 'bin/cargo.exe'
    $env:CARGO_HOME = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
    $env:LIB = 'C:\tools\xwin\crt\lib\x86_64;C:\tools\xwin\sdk\lib\ucrt\x86_64;C:\tools\xwin\sdk\lib\um\x86_64'
    $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER = Join-Path $scopeToolchain 'lib/rustlib/x86_64-pc-windows-msvc/bin/rust-lld.exe'
    $env:CARGO_TARGET_DIR = 'C:\gt\gridtimer-build\sourcegen'
    $env:CARGO_INCREMENTAL = '0'
    $scopeCargoBase = @('test', '--manifest-path', (Join-Path $scopeCopyCrate 'Cargo.toml'), '--locked', '--offline', '--config', 'profile.test.package.gridtimer_native.debug=0', '--lib')
    foreach ($variant in $scopeVariants) {
        $caseDirectory = Join-Path $scopeEvidence $variant.name
        if (Test-Path -LiteralPath $caseDirectory) { throw 'Refusing to overwrite mutation case evidence' }
        [IO.Directory]::CreateDirectory($caseDirectory) | Out-Null
        $caseSource = Join-Path $scopeCopyCrate 'src/legal_scan.rs'
        [IO.File]::WriteAllText($caseSource, $variant.source, [Text.UTF8Encoding]::new($false))
        $compileLog = Join-Path $caseDirectory 'compile.log'
        & $scopeCargo @scopeCargoBase --no-run *> $compileLog
        $compileExit = $LASTEXITCODE
        $testExit = $null
        $allObserved = $false
        $requiredFailureObserved = $false
        $assertionFailureObserved = $false
        $testLogHash = $null
        $passed = $false
        if ($compileExit -eq 0) {
            $testLog = Join-Path $caseDirectory 'tests.log'
            & $scopeCargo @scopeCargoBase latest_scope_ -- --test-threads=1 *> $testLog
            $testExit = $LASTEXITCODE
            $testText = [IO.File]::ReadAllText($testLog)
            $testLogHash = (Get-FileHash -LiteralPath $testLog -Algorithm SHA256).Hash.ToLowerInvariant()
            $allObserved = $true
            foreach ($testName in $scopeTestNames) {
                if ($testText -notmatch ('(?m)^test legal_scan::current_content_tests::' + [regex]::Escape($testName) + ' \.\.\. (ok|FAILED)\s*$')) {
                    $allObserved = $false
                }
            }
            $assertionFailureObserved = $testText.Contains('panicked at') -and $testText.Contains('assertion') -and $testText.Contains('test result: FAILED')
            if ($variant.requiredFailure) {
                $requiredFailureObserved = $testText -match ('(?m)^test legal_scan::current_content_tests::' + [regex]::Escape($variant.requiredFailure) + ' \.\.\. FAILED\s*$')
            }
            $passed = $allObserved -and $(if ($variant.expectFailure) {
                $testExit -ne 0 -and $requiredFailureObserved -and $assertionFailureObserved -and !$testText.Contains('could not compile')
            } else {
                $testExit -eq 0 -and $testText.Contains('6 passed; 0 failed')
            })
        }
        $scopeResults += [ordered]@{
            name=$variant.name;compileExit=$compileExit;testExit=$testExit;passed=$passed
            expectedFailure=$variant.expectFailure;requiredFailureTest=$variant.requiredFailure
            requiredFailureObserved=$requiredFailureObserved;assertionFailureObserved=$assertionFailureObserved
            allBusinessTestsObserved=$allObserved;sourceSha256=(Get-FileHash -LiteralPath $caseSource -Algorithm SHA256).Hash.ToLowerInvariant()
            compileLogSha256=(Get-FileHash -LiteralPath $compileLog -Algorithm SHA256).Hash.ToLowerInvariant();testLogSha256=$testLogHash
        }
        Write-Output "Legal latest scope $($variant.name): compile=$compileExit test=$testExit passed=$passed"
        if (!$passed) { throw "Unexpected mutation result: $($variant.name). See $caseDirectory" }
    }
} finally {
    foreach ($name in $scopeEnvironmentNames) { [Environment]::SetEnvironmentVariable($name, $scopeOldEnvironment[$name], 'Process') }
    $scopeProductionUnchanged = $true
    foreach ($record in $scopeInputRecords) {
        $path = Join-Path $scopeRoot $record.path
        if (!(Test-Path -LiteralPath $path -PathType Leaf) -or (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -cne $record.sha256) {
            $scopeProductionUnchanged = $false
        }
    }
    $scopeSourceHashAfter = (Get-FileHash -LiteralPath $scopeSource -Algorithm SHA256).Hash.ToLowerInvariant()
    $scopePassed = $scopeProductionUnchanged -and $scopeSourceHash -ceq $scopeSourceHashAfter -and $scopeResults.Count -eq 4 -and @($scopeResults | Where-Object { !$_.passed }).Count -eq 0
    $scopeReceipt = [ordered]@{
        passed=$scopePassed;version=$Version;runId=$scopeRunId;sourcePath=$scopeRelativeSource
        sourceSha256Before=$scopeSourceHash;sourceSha256After=$scopeSourceHashAfter;testsSha256=$scopeTestsHash
        productionUnchanged=$scopeProductionUnchanged;verifierSha256=$scopeVerifierHash;tests=$scopeTestNames.Count
        testNames=$scopeTestNames;cases=$scopeResults;copiedInputs=$scopeInputRecords
        hostOnly=$true;networkRequests=0;noDeviceOperations=$true;checkedAt=[DateTimeOffset]::Now.ToString('o')
    }
    $receiptPath = Join-Path $scopeEvidence 'receipt.json'
    if (Test-Path -LiteralPath $receiptPath) { throw 'Refusing to overwrite mutation receipt' }
    [IO.File]::WriteAllText($receiptPath, ($scopeReceipt | ConvertTo-Json -Depth 12), [Text.UTF8Encoding]::new($false))
    Write-Output "Legal latest scope receipt: $receiptPath"
    if (!$scopeProductionUnchanged) { throw 'Production input hashes changed during isolated mutation verification' }
}
if (!$scopePassed) { throw 'Legal latest scope mutation verification failed' }
