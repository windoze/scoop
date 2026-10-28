#ifndef M13_CALLBACK_THREADING_H
#define M13_CALLBACK_THREADING_H

/* Native threads deliberately enter Scoop only through the callback under test. */
#ifdef _WIN32
#include <process.h>
#include <stdint.h>
#include <windows.h>

typedef HANDLE M13Thread;
typedef SRWLOCK M13Mutex;
typedef CONDITION_VARIABLE M13Condition;
typedef unsigned M13ThreadResult;
#define M13_THREAD_CALL __stdcall

static inline int m13_mutex_init(M13Mutex *mutex) {
    InitializeSRWLock(mutex);
    return 0;
}

static inline int m13_mutex_lock(M13Mutex *mutex) {
    AcquireSRWLockExclusive(mutex);
    return 0;
}

static inline int m13_mutex_unlock(M13Mutex *mutex) {
    ReleaseSRWLockExclusive(mutex);
    return 0;
}

static inline int m13_mutex_destroy(M13Mutex *mutex) {
    /* SRW locks have no resources to release. */
    (void)mutex;
    return 0;
}

static inline int m13_condition_init(M13Condition *condition) {
    InitializeConditionVariable(condition);
    return 0;
}

static inline int m13_condition_wait(M13Condition *condition, M13Mutex *mutex) {
    return SleepConditionVariableSRW(condition, mutex, INFINITE, 0) ? 0 : -1;
}

static inline int m13_condition_signal(M13Condition *condition) {
    WakeConditionVariable(condition);
    return 0;
}

static inline int m13_condition_broadcast(M13Condition *condition) {
    WakeAllConditionVariable(condition);
    return 0;
}

static inline int m13_condition_destroy(M13Condition *condition) {
    /* Windows condition variables have no resources to release. */
    (void)condition;
    return 0;
}

static inline int m13_thread_create(M13Thread *thread,
                                    M13ThreadResult(M13_THREAD_CALL *entry)(void *),
                                    void *argument) {
    uintptr_t handle = _beginthreadex(NULL, 0, entry, argument, 0, NULL);
    if (handle == 0) {
        return -1;
    }
    *thread = (HANDLE)handle;
    return 0;
}

static inline int m13_thread_join(M13Thread thread) {
    DWORD result = WaitForSingleObject(thread, INFINITE);
    BOOL closed = CloseHandle(thread);
    return result == WAIT_OBJECT_0 && closed ? 0 : -1;
}

static inline void m13_thread_yield(void) { SwitchToThread(); }

#else
#include <pthread.h>
#include <sched.h>

typedef pthread_t M13Thread;
typedef pthread_mutex_t M13Mutex;
typedef pthread_cond_t M13Condition;
typedef void *M13ThreadResult;
#define M13_THREAD_CALL

#define m13_mutex_init(mutex) pthread_mutex_init(mutex, NULL)
#define m13_mutex_lock pthread_mutex_lock
#define m13_mutex_unlock pthread_mutex_unlock
#define m13_mutex_destroy pthread_mutex_destroy
#define m13_condition_init(condition) pthread_cond_init(condition, NULL)
#define m13_condition_wait pthread_cond_wait
#define m13_condition_signal pthread_cond_signal
#define m13_condition_broadcast pthread_cond_broadcast
#define m13_condition_destroy pthread_cond_destroy
#define m13_thread_create(thread, entry, argument) pthread_create(thread, NULL, entry, argument)
#define m13_thread_join(thread) pthread_join(thread, NULL)
#define m13_thread_yield sched_yield
#endif

#endif
