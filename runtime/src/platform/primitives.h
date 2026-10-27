#ifndef SCOOP_PLATFORM_PRIMITIVES_H
#define SCOOP_PLATFORM_PRIMITIVES_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#if defined(_WIN32)
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>
typedef SRWLOCK ScoopPlatformMutex;
typedef CONDITION_VARIABLE ScoopPlatformCondition;
typedef INIT_ONCE ScoopPlatformOnce;
typedef HANDLE ScoopPlatformThread;
#define SCOOP_PLATFORM_MUTEX_INIT SRWLOCK_INIT
#define SCOOP_PLATFORM_CONDITION_INIT CONDITION_VARIABLE_INIT
#define SCOOP_PLATFORM_ONCE_INIT INIT_ONCE_STATIC_INIT
#else
#include <pthread.h>
typedef pthread_mutex_t ScoopPlatformMutex;
typedef pthread_cond_t ScoopPlatformCondition;
typedef pthread_once_t ScoopPlatformOnce;
typedef pthread_t ScoopPlatformThread;
#define SCOOP_PLATFORM_MUTEX_INIT PTHREAD_MUTEX_INITIALIZER
#define SCOOP_PLATFORM_CONDITION_INIT PTHREAD_COND_INITIALIZER
#define SCOOP_PLATFORM_ONCE_INIT PTHREAD_ONCE_INIT
#endif

/* Zero denotes success. Timed waits report ETIMEDOUT after reacquiring the lock. */
int scoop_platform_mutex_init(ScoopPlatformMutex *mutex);
int scoop_platform_mutex_lock(ScoopPlatformMutex *mutex);
int scoop_platform_mutex_unlock(ScoopPlatformMutex *mutex);
void scoop_platform_mutex_destroy(ScoopPlatformMutex *mutex);
int scoop_platform_condition_wait(ScoopPlatformCondition *condition, ScoopPlatformMutex *mutex);
int scoop_platform_condition_wait_for(ScoopPlatformCondition *condition, ScoopPlatformMutex *mutex,
                                      uint32_t microseconds);
int scoop_platform_condition_signal(ScoopPlatformCondition *condition);
int scoop_platform_condition_broadcast(ScoopPlatformCondition *condition);
int scoop_platform_once(ScoopPlatformOnce *once, void (*initialize)(void));
int scoop_platform_thread_create(ScoopPlatformThread *thread, void *(*entry)(void *),
                                 void *argument);
int scoop_platform_thread_join(ScoopPlatformThread thread);
void *scoop_platform_aligned_allocate(size_t alignment, size_t size);
void scoop_platform_aligned_free(void *pointer);
bool scoop_platform_monotonic_ns(uint64_t *value);
bool scoop_platform_thread_cpu_ns(uint64_t *value);

#endif
