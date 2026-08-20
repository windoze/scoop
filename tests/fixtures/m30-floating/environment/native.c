#include <fenv.h>
#include <pthread.h>
#include <stdint.h>
#include <stdlib.h>
#if defined(__x86_64__)
#include <xmmintrin.h>
#endif

static void dirty_environment(void) {
    if (fesetround(FE_UPWARD) != 0) abort();
#if defined(__x86_64__)
    _mm_setcsr((_mm_getcsr() | UINT32_C(0x8040)) & ~UINT32_C(0x80));
#elif defined(__aarch64__)
    uint64_t fpcr;
    __asm__ volatile("mrs %0, fpcr" : "=r"(fpcr));
    fpcr |= UINT64_C(1) << 24;
    __asm__ volatile("msr fpcr, %0" : : "r"(fpcr));
#endif
}

int32_t environment_ready(void) {
    if (fegetround() != FE_TONEAREST) return 0;
#if defined(__x86_64__)
    uint32_t csr = _mm_getcsr();
    return (csr & UINT32_C(0x8040)) == 0 &&
           (csr & UINT32_C(0x1f80)) == UINT32_C(0x1f80);
#elif defined(__aarch64__)
    uint64_t fpcr;
    __asm__ volatile("mrs %0, fpcr" : "=r"(fpcr));
    return (fpcr & ((UINT64_C(1) << 24) | UINT64_C(0x9f00))) == 0;
#endif
}

float single_seed(int32_t kind) {
    return kind == 0 ? 16777216.0f : 0x1p-126f;
}
double double_seed(int32_t kind) {
    return kind == 0 ? 9007199254740992.0 : 0x1p-1022;
}

typedef double (*Callback)(float, double, void *);
typedef struct { Callback callback; void *context; double result; } Job;
static void *worker(void *opaque) {
    Job *job = opaque;
    dirty_environment();
    job->result = job->callback(single_seed(0), double_seed(0), job->context);
    return NULL;
}

double run_foreign(Callback callback, void *context) {
    Job job = {callback, context, 0.0};
    pthread_t thread;
    if (pthread_create(&thread, NULL, worker, &job) != 0 ||
        pthread_join(thread, NULL) != 0) abort();
    return job.result;
}
