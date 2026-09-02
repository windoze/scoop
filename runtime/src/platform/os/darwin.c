#include <pthread.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>

#include "../platform.h"

#if !defined(__APPLE__) || !defined(__aarch64__)
#error "the Darwin/AArch64 platform component requires macOS on AArch64"
#endif

ScoopPlatformStackBounds scoop_platform_stack_bounds(void) {
    pthread_t self = pthread_self();
    void *stack_high = pthread_get_stackaddr_np(self);
    size_t stack_size = pthread_get_stacksize_np(self);
    if (stack_high == NULL || stack_size == 0) {
        fprintf(stderr, "scoop runtime: failed to read pthread stack bounds\n");
        abort();
    }
    ScoopPlatformStackBounds bounds = {
        .low = (const char *)stack_high - stack_size,
        .high = stack_high,
    };
    return bounds;
}
