# Android 2.23.2.50 Rust knowledge filter trial and startup mirror memo

Candidate version 2.23.2.50, versionCode 22319. This is an experimental Android-only PR from main 2d8c20e. It does not modify Windows, sync protocol, storage schema, authentication, or encrypted note policy. Do not merge until CI and device acceptance have passed.

## Changes

1. Knowledge quick-filter counters now select the active workspace/folder and apply the query only once. They pass four boolean metadata bits for each eligible document in one bounded JNI call to Rust. Native returns the counters for ALL, PINNED, TODO, IMAGE and UNFILED. JVM/test/unsupported cases use the same one-pass Kotlin metadata fallback. Locked documents remain eligible only for ALL. No plaintext bodies, account identifiers or decrypted content are retained in Rust.
2. StartupSnapshotDecodeScope probes same-string identity before exact content equality and reuses the first matching decoded variant. It avoids a redundant full-string comparison for the common primary/backup mirror, and removes a temporary version-list allocation. Distinct schema declarations still require distinct decoding; future schemas, failures and recovery priorities are unchanged.
3. Add Rust unit tests for the native counter and generated JVM regressions for counts/scope and schema variant reuse.

## Risk and performance acceptance

This is a trial, not a verified speedup. Marshaling the metadata array over JNI and Kotlin search remain measurable costs. Use existing startup and knowledge profile probes to compare to the main branch on the same phone and dataset. Acceptance requires Rust source-generation tests, release JVM tests, lint, signed release APK build and real-device checks for empty/large workspaces, locked notes, quick filter counts, startup recovery, future schema and retained histories. Cold launch must not bypass any snapshot verification. CI failure blocks release claims.

A passing PR alone does not mean an APK was installed or the 3–5 second launch problem is solved. Publish only verified signed binaries via the existing workflow.
