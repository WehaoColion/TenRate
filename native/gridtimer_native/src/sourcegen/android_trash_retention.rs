// Android-only retention scheduling. Source of truth stays in Rust; this module
// emits the small Android OS integration and patches the existing repository.
const BRIDGE: &str = "com/ofairyo/gridtimer/core/NativeOptimizerBridge.kt";
const REPOSITORY: &str = "com/ofairyo/gridtimer/data/TimerRepository.kt";
const ACTIVITY: &str = "com/ofairyo/gridtimer/MainActivity.kt";
const NOTE_STUDIO: &str = "com/ofairyo/gridtimer/ui/NoteStudioSheet.kt";

pub const JOB_PATH: &str = "com/ofairyo/gridtimer/data/NoteTrashRetentionJobService.kt";
pub const JOB_CONTENTS: &str = r####"package com.ofairyo.gridtimer.data

import android.app.job.JobInfo
import android.app.job.JobParameters
import android.app.job.JobScheduler
import android.app.job.JobService
import android.content.ComponentName
import android.content.Context
import android.util.Log
import com.ofairyo.gridtimer.GridTimerApplication
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import java.util.concurrent.TimeUnit

// JobScheduler is deliberately inexact under Doze; the repository also checks
// asynchronously at startup and after synchronization for overdue entries.
class NoteTrashRetentionJobService : JobService() {
    @Volatile private var stopped = false
    private var running: Job? = null

    override fun onStartJob(params: JobParameters): Boolean {
        stopped = false
        val workScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
        running = workScope.launch {
            try {
                GridTimerApplication.timerRepository(applicationContext).purgeExpiredNoteTrash()
            } catch (_: CancellationException) {
                // The scheduler will retry a stopped job.
            } catch (failure: Exception) {
                Log.w("TrashRetention", "Scheduled purge deferred", failure)
            } finally {
                if (!stopped) jobFinished(params, false)
                workScope.cancel()
            }
        }
        return true
    }

    override fun onStopJob(params: JobParameters): Boolean {
        stopped = true
        running?.cancel()
        running = null
        return true
    }

    companion object {
        private const val JOB_ID = 0x54524153
        private val PERIOD_MILLIS = TimeUnit.DAYS.toMillis(1)

        fun schedule(context: Context) {
            runCatching {
                val scheduler = context.getSystemService(Context.JOB_SCHEDULER_SERVICE)
                    as? JobScheduler ?: return
                if (scheduler.getPendingJob(JOB_ID) != null) return
                val job = JobInfo.Builder(
                    JOB_ID,
                    ComponentName(context, NoteTrashRetentionJobService::class.java)
                ).setPersisted(true)
                    .setPeriodic(PERIOD_MILLIS)
                    .build()
                scheduler.schedule(job)
            }.onFailure { failure ->
                Log.w("TrashRetention", "Could not schedule periodic purge", failure)
            }
        }
    }
}
"####;

fn replace_once(source: &mut String, anchor: &str, replacement: &str) -> Result<(), String> {
    if source.matches(anchor).count() != 1 {
        return Err(format!("trash retention source anchor absent or repeated: {anchor}"));
    }
    *source = source.replacen(anchor, replacement, 1);
    Ok(())
}

pub fn render(path: &str, base: &str) -> Result<String, String> {
    let mut source = base.to_owned();
    match path {
        BRIDGE => {
            replace_once(
                &mut source,
                "    fun emptyNoteTrashAppDataJson(appDataJson: String, now: Long): String? {",
                r####"    // A null result is either a no-op or a native failure; callers prefilter
    // the eligible rows and fail closed if native execution is unavailable.
    fun purgeExpiredNoteTrashAppDataJson(appDataJson: String, now: Long): String? {
        if (!nativeAvailable) return null
        return runCatching {
            nativePurgeExpiredNoteTrashAppDataJson(appDataJson, now)
        }.getOrNull()
    }

    fun emptyNoteTrashAppDataJson(appDataJson: String, now: Long): String? {"####,
            )?;
            replace_once(
                &mut source,
                "    @JvmStatic\n    private external fun nativeEmptyNoteTrashAppDataJson(",
                r####"    @JvmStatic
    private external fun nativePurgeExpiredNoteTrashAppDataJson(
        appDataJson: String,
        now: Long
    ): String?

    @JvmStatic
    private external fun nativeEmptyNoteTrashAppDataJson("####,
            )?;
        }
        REPOSITORY => {
            replace_once(
                &mut source,
                "    private val accountBootstrapJob = scope.launch {",
                r####"    // Never delay the first screen: expiry runs on the repository IO scope
    // after the verified workspace load is complete.
    private val trashRetentionStartupJob = scope.launch {
        initializationJob.join()
        purgeExpiredNoteTrash()
    }
    private val accountBootstrapJob = scope.launch {"####,
            )?;
            replace_once(
                &mut source,
                "    suspend fun emptyNoteTrash(expectedWorkspaceKey: String): Boolean {",
                r####"    suspend fun purgeExpiredNoteTrash(expectedWorkspaceKey: String? = null): Boolean {
        awaitInitialized()
        val workspace = expectedWorkspaceKey ?: activeWorkspaceKey
        val currentTime = now()
        val cutoff = currentTime - 100L * 24L * 60L * 60L * 1000L
        if (cutoff < 0L || _appData.value.notes.none {
                it.deletedAtEpochMillis?.let { deleted ->
                    deleted >= 0L && deleted <= cutoff
                } == true
            }) return false

        // Check the workspace again under the existing write mutex, then
        // publish only after a successful durable commit. No Kotlin fallback
        // may bypass Rust's attachment ownership and tombstone safeguards.
        var changed = false
        val attempt = updateDataDetailed(
            expectedWorkspaceKey = workspace,
            persistImmediately = true,
            publishAfterDurable = true
        ) { data ->
            val timestamp = now()
            val threshold = timestamp - 100L * 24L * 60L * 60L * 1000L
            if (threshold < 0L || data.notes.none {
                    it.deletedAtEpochMillis?.let { deleted ->
                        deleted >= 0L && deleted <= threshold
                    } == true
                }) return@updateDataDetailed data
            val raw = NativeOptimizerBridge.purgeExpiredNoteTrashAppDataJson(
                json.encodeToString(data), timestamp
            ) ?: error("Native 100-day trash purge failed; original state retained")
            val updated = decodeNativeAppData(raw)
                ?: error("Native 100-day trash purge returned an unreadable snapshot")
            changed = updated != data
            updated
        }
        if (attempt.succeeded && changed) {
            attempt.snapshot?.let { snapshot -> pruneNoteMedia(snapshot) }
        }
        return attempt.succeeded && changed
    }

    suspend fun emptyNoteTrash(expectedWorkspaceKey: String): Boolean {"####,
            )?;
            replace_once(
                &mut source,
                "    suspend fun syncNow() {\n        accountSyncMutex.withLock { syncNowLocked() }\n    }",
                r####"    suspend fun syncNow() {
        accountSyncMutex.withLock { syncNowLocked() }
        purgeExpiredNoteTrash()
    }"####,
            )?;
            replace_once(
                &mut source,
                "            syncNowLocked()\n        } finally {\n            accountSyncMutex.unlock()",
                "            syncNowLocked()\n            purgeExpiredNoteTrash()\n        } finally {\n            accountSyncMutex.unlock()",
            )?;
        }
        ACTIVITY => {
            replace_once(
                &mut source,
                "        super.onCreate()\n        DiagnosticLogStore.install(this)",
                "        super.onCreate()\n        NoteTrashRetentionJobService.schedule(this)\n        DiagnosticLogStore.install(this)",
            )?;
            replace_once(
                &mut source,
                "import com.ofairyo.gridtimer.data.TimerRepository",
                "import com.ofairyo.gridtimer.data.NoteTrashRetentionJobService\nimport com.ofairyo.gridtimer.data.TimerRepository",
            )?;
        }
        NOTE_STUDIO => {
            replace_once(
                &mut source,
                r####"subtitle = if (trashMode) "已删除页面" else "本地知识页","####,
                r####"subtitle = if (trashMode) "保留 100 天，到期自动清除" else "本地知识页","####,
            )?;
            replace_once(
                &mut source,
                r####"text = "误删内容会在这里。","####,
                r####"text = "回收站内容保留 100 天，到期自动清除。","####,
            )?;
            replace_once(
                &mut source,
                r####""删掉的文档会先来到这里，之后随时可以恢复。""####,
                r####""删除后的文档保留 100 天，到期自动永久清除。""####,
            )?;
        }
        _ => {}
    }
    Ok(source)
}

pub fn render_manifest(path: &str, base: &str) -> Result<String, String> {
    if path != "AndroidManifest.xml" {
        return Ok(base.to_owned());
    }
    let mut source = base.to_owned();
    replace_once(
        &mut source,
        "    <uses-permission android:name=\"android.permission.WAKE_LOCK\" />",
        "    <uses-permission android:name=\"android.permission.WAKE_LOCK\" />\n    <uses-permission android:name=\"android.permission.RECEIVE_BOOT_COMPLETED\" />",
    )?;
    replace_once(
        &mut source,
        "        <receiver\n            android:name=\".notifications.TimerNotificationActionReceiver\"",
        "        <service\n            android:name=\".data.NoteTrashRetentionJobService\"\n            android:permission=\"android.permission.BIND_JOB_SERVICE\"\n            android:exported=\"true\" />\n\n        <receiver\n            android:name=\".notifications.TimerNotificationActionReceiver\"",
    )?;
    Ok(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(path: &str) -> &'static str {
        super::super::kotlin_sources::SOURCES.iter()
            .find(|source| source.path == path).unwrap().contents
    }

    #[test]
    fn builds_android_retention_integration_without_touching_windows() {
        let bridge = render(BRIDGE, base(BRIDGE)).unwrap();
        assert!(bridge.contains("nativePurgeExpiredNoteTrashAppDataJson"));
        let repo = render(REPOSITORY, base(REPOSITORY)).unwrap();
        assert!(repo.contains("publishAfterDurable = true"));
        assert!(repo.contains("purgeExpiredNoteTrash()"));
        let activity = render(ACTIVITY, base(ACTIVITY)).unwrap();
        assert!(activity.contains("NoteTrashRetentionJobService.schedule(this)"));
        let manifest = super::super::android_sources::SOURCES.iter()
            .find(|source| source.path == "AndroidManifest.xml").unwrap();
        assert!(render_manifest(manifest.path, manifest.contents).unwrap()
            .contains("android.permission.BIND_JOB_SERVICE"));
    }

    #[test]
    fn generator_fails_closed_when_the_integration_anchor_changes() {
        assert!(render(REPOSITORY, "other repository layout").is_err());
        assert!(render_manifest("AndroidManifest.xml", "<manifest/>").is_err());
        assert_eq!(render("untouched", "same").unwrap(), "same");
    }
}
