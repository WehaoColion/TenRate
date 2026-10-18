# v0.0.1 - Compile the real Rust-owned export helper and exercise raster acceptance mutations.
"""Offline host verification; does not simulate Android WebView or phone rendering."""
from pathlib import Path
import hashlib
import json
import os
import re
import subprocess

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / 'native/gridtimer_native/src/sourcegen/android_document_exit_feedback.rs'
OUT = ROOT / 'tmp/share_image_verification'
RAW = SOURCE.read_text(encoding='utf-8')
HASH = hashlib.sha256(SOURCE.read_bytes()).hexdigest()

def literal(name):
    found = re.findall(r'(?:pub\s+)?const\s+' + re.escape(name) + r':\s*&str\s*=\s*r####"(.*?)"####;', RAW, re.S)
    assert len(found) == 1, f'Missing or ambiguous Rust literal: {name}'
    return found[0]

def dependency(coordinate, version, name):
    root = Path.home() / '.gradle/caches/modules-2/files-2.1' / coordinate / version
    found = list(root.rglob(name))
    assert len(found) == 1, f'Missing cached dependency: {name}'
    return str(found[0])

compiler = [dependency(*d) for d in [
    ('org.jetbrains.kotlin/kotlin-compiler-embeddable', '1.9.24', 'kotlin-compiler-embeddable-1.9.24.jar'),
    ('org.jetbrains.kotlin/kotlin-stdlib', '1.9.24', 'kotlin-stdlib-1.9.24.jar'),
    ('org.jetbrains.kotlin/kotlin-script-runtime', '1.9.24', 'kotlin-script-runtime-1.9.24.jar'),
    ('org.jetbrains.kotlin/kotlin-reflect', '1.6.10', 'kotlin-reflect-1.6.10.jar'),
    ('org.jetbrains.kotlin/kotlin-daemon-embeddable', '1.9.24', 'kotlin-daemon-embeddable-1.9.24.jar'),
    ('org.jetbrains.intellij.deps/trove4j', '1.0.20200330', 'trove4j-1.0.20200330.jar'),
    ('org.jetbrains/annotations', '13.0', 'annotations-13.0.jar'),
]]
runtime = [dependency(*d) for d in [
    ('org.jetbrains.kotlin/kotlin-stdlib', '1.9.24', 'kotlin-stdlib-1.9.24.jar'),
    ('org.jetbrains.kotlinx/kotlinx-coroutines-core-jvm', '1.8.1', 'kotlinx-coroutines-core-jvm-1.8.1.jar'),
    ('junit/junit', '4.13.2', 'junit-4.13.2.jar'),
    ('org.hamcrest/hamcrest-core', '1.3', 'hamcrest-core-1.3.jar'),
    ('org.jetbrains/annotations', '23.0.0', 'annotations-23.0.0.jar'),
]]
JAVA = 'C:/tools/java/jdk-17.0.18+8/bin/java.exe'
ANDROID = 'C:/tools/android-sdk/platforms/android-35/android.jar'

def run(args, log):
    result = subprocess.run([JAVA, *args], stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                            timeout=90, creationflags=subprocess.CREATE_NO_WINDOW)
    log.write_bytes(result.stdout)
    return result.returncode, result.stdout.decode('utf-8', errors='replace')

policy = literal('SHARE_RASTER_POLICY')
helper = literal('SHARE_IMAGE_HELPER')
tests = literal('SHARE_RASTER_TESTS')
# Compile-only ports are never run. Android APIs are provided by the actual SDK;
# the tests execute only the real pure pixel acceptance function.
ports = '''package com.ofairyo.gridtimer.ui
internal const val ANDROID_MARKDOWN_DOCUMENT_PREFIX = "https://invalid.example/"
internal fun androidMarkdownDocumentAllowed(url: String?, documentUrl: String): Boolean = error("compile-only port")
internal fun androidMarkdownResourceAllowed(url: String, documentUrl: String, main: Boolean): Boolean = error("compile-only port")
'''
bridge = '''package com.ofairyo.gridtimer.core
internal object NativeOptimizerBridge {
    fun renderAndroidAiAnswerHtml(content: String, dark: Boolean): String? = error("compile-only port")
}
'''
results = []
for case in ['baseline', 'cosmetic', 'removed_blank_guard', 'removed_size_guard', 'restored']:
    current = policy
    if case == 'cosmetic':
        current = current.replace('// Export acceptance, independent of DOM readiness and bitmap allocation.', '// Image acceptance.')
    if case == 'removed_blank_guard':
        current = current.replace('    return false', '    return true')
    if case == 'removed_size_guard':
        current = current.replace('if (!shareRasterSizeAllowed(width, height.toLong())) return false', 'if (width == 0) return true')
    folder = OUT / case
    folder.mkdir(parents=True, exist_ok=True)
    files = {'ShareRaster.kt': 'package com.ofairyo.gridtimer.ui\nimport kotlin.math.roundToInt\n' + helper + current,
             'ShareRasterPolicyTest.kt': tests, 'Ports.kt': ports, 'Bridge.kt': bridge}
    paths = []
    for name, content in files.items():
        path = folder / name
        path.write_text(content, encoding='utf-8')
        paths.append(str(path))
    classes = str(folder / 'classes')
    compile_exit, output = run(['-Xmx384m', '-XX:ActiveProcessorCount=2', '-cp', os.pathsep.join(compiler),
        'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler', '-no-stdlib', '-no-reflect', '-jvm-target', '17',
        '-classpath', os.pathsep.join(runtime + [ANDROID]), '-d', classes, *paths], folder / 'compile.log')
    assert compile_exit == 0, output
    test_exit, output = run(['-Xmx192m', '-cp', os.pathsep.join([classes, *runtime]), 'org.junit.runner.JUnitCore',
                             'com.ofairyo.gridtimer.ui.ShareRasterPolicyTest'], folder / 'tests.log')
    expected = case not in ['removed_blank_guard', 'removed_size_guard']
    named_failure = ('allocatedButUnpaintedFrameIsRejected' if case == 'removed_blank_guard'
                     else 'dimensionsFailClosedBeforeReadingPixels')
    passed = ((test_exit == 0 and 'OK (4 tests)' in output) if expected
              else test_exit != 0 and 'java.lang.AssertionError' in output and 'Tests run: 4' in output and named_failure in output)
    assert passed, f'{case}: {output}'
    results.append(dict(name=case, compileExit=compile_exit, testExit=test_exit, expectedPass=expected, passed=passed))
    print(f'{case} verified: compile={compile_exit} test={test_exit}', flush=True)
assert hashlib.sha256(SOURCE.read_bytes()).hexdigest() == HASH
(OUT / 'receipt.json').write_text(json.dumps(dict(passed=True, sourceSha256=HASH, tests=4, cases=results,
    deviceVerified=False, scope='Actual Rust-owned helper compiled against Android 35 and coroutines. Pure raster acceptance tests only; no WebView painting, screenshot, or phone validation.'), indent=2), encoding='utf-8')
