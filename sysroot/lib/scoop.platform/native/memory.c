#include <errno.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

void *scoop_platform_memory_resize(void *pointer, int64_t capacity) {
    if (capacity <= 0 || (uint64_t)capacity > SIZE_MAX || (uint64_t)capacity > PTRDIFF_MAX) {
        errno = ENOMEM;
        return NULL;
    }
    return realloc(pointer, (size_t)capacity);
}

void scoop_platform_memory_free(void *pointer) { free(pointer); }

void scoop_platform_memory_move(void *destination, const void *source, int64_t count) {
    memmove(destination, source, (size_t)count);
}
