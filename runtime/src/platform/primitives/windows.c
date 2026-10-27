#include "../primitives.h"
#include <errno.h>
#include <malloc.h>
#include <process.h>
#include <stdlib.h>

int scoop_platform_mutex_init(ScoopPlatformMutex *mutex) {
    InitializeSRWLock(mutex);
    return 0;
}
int scoop_platform_mutex_lock(ScoopPlatformMutex *mutex) {
    AcquireSRWLockExclusive(mutex);
    return 0;
}
int scoop_platform_mutex_unlock(ScoopPlatformMutex *mutex) {
    ReleaseSRWLockExclusive(mutex);
    return 0;
}
void scoop_platform_mutex_destroy(ScoopPlatformMutex *mutex) {
    /* SRW locks own no external resource and need no destruction operation. */
    (void)mutex;
}
static int wait_condition(ScoopPlatformCondition *condition, ScoopPlatformMutex *mutex,
                          DWORD milliseconds) {
    if (SleepConditionVariableSRW(condition, mutex, milliseconds, 0))
        return 0;
    DWORD error = GetLastError();
    return error == ERROR_TIMEOUT ? ETIMEDOUT : (int)error;
}
int scoop_platform_condition_wait(ScoopPlatformCondition *condition, ScoopPlatformMutex *mutex) {
    return wait_condition(condition, mutex, INFINITE);
}
int scoop_platform_condition_wait_for(ScoopPlatformCondition *condition, ScoopPlatformMutex *mutex,
                                      uint32_t microseconds) {
    return wait_condition(condition, mutex, microseconds / 1000 + (microseconds % 1000 != 0));
}
int scoop_platform_condition_signal(ScoopPlatformCondition *condition) {
    WakeConditionVariable(condition);
    return 0;
}
int scoop_platform_condition_broadcast(ScoopPlatformCondition *condition) {
    WakeAllConditionVariable(condition);
    return 0;
}
typedef struct OnceCall {
    void (*initialize)(void);
} OnceCall;
static BOOL CALLBACK run_once(INIT_ONCE *once, void *parameter, void **context) {
    (void)once;
    (void)context;
    ((OnceCall *)parameter)->initialize();
    return TRUE;
}
int scoop_platform_once(ScoopPlatformOnce *once, void (*initialize)(void)) {
    OnceCall call = {initialize};
    return InitOnceExecuteOnce(once, run_once, &call, NULL) ? 0 : (int)GetLastError();
}
typedef struct ThreadCall {
    void *(*entry)(void *);
    void *argument;
} ThreadCall;
static unsigned __stdcall run_thread(void *parameter) {
    ThreadCall call = *(ThreadCall *)parameter;
    free(parameter);
    call.entry(call.argument);
    return 0;
}
int scoop_platform_thread_create(ScoopPlatformThread *thread, void *(*entry)(void *),
                                 void *argument) {
    ThreadCall *call = malloc(sizeof *call);
    if (call == NULL)
        return ENOMEM;
    *call = (ThreadCall){entry, argument};
    uintptr_t handle = _beginthreadex(NULL, 0, run_thread, call, 0, NULL);
    if (handle == 0) {
        int error = errno;
        free(call);
        return error;
    }
    *thread = (HANDLE)handle;
    return 0;
}
int scoop_platform_thread_join(ScoopPlatformThread thread) {
    if (WaitForSingleObject(thread, INFINITE) != WAIT_OBJECT_0)
        return (int)GetLastError();
    return CloseHandle(thread) ? 0 : (int)GetLastError();
}
void *scoop_platform_aligned_allocate(size_t alignment, size_t size) {
    return _aligned_malloc(size, alignment);
}
void scoop_platform_aligned_free(void *pointer) { _aligned_free(pointer); }
bool scoop_platform_monotonic_ns(uint64_t *value) {
    LARGE_INTEGER counter, frequency;
    if (!QueryPerformanceFrequency(&frequency) || !QueryPerformanceCounter(&counter))
        return false;
    uint64_t ticks = (uint64_t)counter.QuadPart, hz = (uint64_t)frequency.QuadPart;
    *value = (ticks / hz) * UINT64_C(1000000000) + (ticks % hz) * UINT64_C(1000000000) / hz;
    return true;
}
bool scoop_platform_thread_cpu_ns(uint64_t *value) {
    FILETIME created, exited, kernel, user;
    if (!GetThreadTimes(GetCurrentThread(), &created, &exited, &kernel, &user))
        return false;
    uint64_t kernel_ticks = (uint64_t)kernel.dwHighDateTime << 32 | kernel.dwLowDateTime;
    uint64_t user_ticks = (uint64_t)user.dwHighDateTime << 32 | user.dwLowDateTime;
    *value = (kernel_ticks + user_ticks) * 100;
    return true;
}
