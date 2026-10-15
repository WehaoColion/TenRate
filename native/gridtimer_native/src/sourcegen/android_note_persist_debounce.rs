// v2.23.2.35 - Cancel superseded snapshot waiters before they acquire the write lock.
// Keep the accepted transaction non-cancellable after the lock is acquired.
const REPOSITORY: &str = "com/ofairyo/gridtimer/data/TimerRepository.kt";
pub const PATH: &str = "com/ofairyo/gridtimer/data/NotePersistDebounce.kt";
pub const TEST_PATH: &str = "com/ofairyo/gridtimer/data/NotePersistDebounceTest.kt";

pub fn render(path: &str, base: &str) -> Result<String, String> {
    if path != REPOSITORY {
        return Ok(base.to_owned());
    }
    if base.matches(OLD_COMMIT).count() != 1 {
        return Err("note persistence debounce commit boundary changed".to_owned());
    }
    Ok(base.replacen(OLD_COMMIT, NEW_COMMIT, 1))
}

const OLD_COMMIT: &str = r####"            // A newer keystroke may cancel the debounce owner, but once SQLite starts
            // writing the transaction must finish. The next debounce will then persist
            // any state that arrived while this commit was in flight.
            val succeeded = withContext(NonCancellable) {
                writeMutex.withLock {
                    flushPersistLocked(_appData.value)
                }
            }"####;

const NEW_COMMIT: &str = r####"            // A superseded debounce must leave the lock queue before starting IO.
            // Once admitted, finish the accepted transaction before releasing the lock.
            val succeeded = withCancellableNotePersistLock(writeMutex) {
                flushPersistLocked(_appData.value)
            }"####;

pub const CONTENTS: &str = r####"package com.ofairyo.gridtimer.data

import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext

// Waiting does not own a transaction. Cancellation may remove a superseded
// debounce from the mutex queue, but must never interrupt an admitted commit.
internal suspend fun <T> withCancellableNotePersistLock(
    writeMutex: Mutex,
    commit: suspend () -> T
): T = writeMutex.withLock {
    withContext(NonCancellable) { commit() }
}
"####;

pub const TEST_CONTENTS: &str = r####"package com.ofairyo.gridtimer.data

import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.sync.Mutex
import org.junit.Assert.*
import org.junit.Test

class NotePersistDebounceTest {
    @Test fun supersededWaiterCannotStartAnotherSnapshotWrite() = runBlocking {
        val mutex = Mutex(locked = true)
        val commits = mutableListOf<String>()
        // UNDISPATCHED reaches the contended lock before launch returns. No clock,
        // storage latency or scheduler timing is needed to reproduce the queue.
        val stale = launch(start = CoroutineStart.UNDISPATCHED) {
            withCancellableNotePersistLock(mutex) { commits += "stale" }
        }
        stale.cancel()
        val latest = launch(start = CoroutineStart.UNDISPATCHED) {
            withCancellableNotePersistLock(mutex) { commits += "latest" }
        }
        mutex.unlock()
        stale.join()
        latest.join()
        assertEquals(listOf("latest"), commits)
        assertFalse(mutex.isLocked)
    }

    @Test fun cancellationAfterAdmissionKeepsTransactionAndLockUntilCommitCompletes() = runBlocking {
        val mutex = Mutex()
        val releaseCommit = CompletableDeferred<Unit>()
        val commits = mutableListOf<String>()
        val accepted = launch(start = CoroutineStart.UNDISPATCHED) {
            withCancellableNotePersistLock(mutex) {
                commits += "started"
                releaseCommit.await()
                commits += "committed"
            }
        }
        accepted.cancel()
        assertTrue(mutex.isLocked)
        assertFalse(accepted.isCompleted)
        val successor = launch(start = CoroutineStart.UNDISPATCHED) {
            withCancellableNotePersistLock(mutex) { commits += "successor" }
        }
        assertEquals(listOf("started"), commits)
        releaseCommit.complete(Unit)
        accepted.join()
        successor.join()
        assertEquals(listOf("started", "committed", "successor"), commits)
        assertFalse(mutex.isLocked)
    }

    @Test fun commitFailureIsReturnedToTheExistingRetryDecision() = runBlocking {
        val mutex = Mutex()
        val result = withCancellableNotePersistLock(mutex) { false }
        assertFalse(result)
        assertFalse(mutex.isLocked)
        assertTrue(withCancellableNotePersistLock(mutex) { true })
    }

    @Test fun commitExceptionPropagatesAndReleasesTheLock() = runBlocking {
        val mutex = Mutex()
        val failure = IllegalStateException("synthetic storage failure")
        var caught: Throwable? = null
        try {
            withCancellableNotePersistLock<Unit>(mutex) { throw failure }
        } catch (throwable: Throwable) {
            caught = throwable
        }
        // Coroutine stack recovery may copy an exception; its error and lock boundary must survive.
        assertEquals(failure.javaClass, caught?.javaClass)
        assertEquals(failure.message, caught?.message)
        assertFalse(mutex.isLocked)
        assertEquals("next", withCancellableNotePersistLock(mutex) { "next" })
    }
}
"####;
