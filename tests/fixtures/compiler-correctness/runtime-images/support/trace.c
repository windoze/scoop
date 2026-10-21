#include <inttypes.h>
#include <stdio.h>

void m23_trace(int64_t value) {
    printf("%" PRId64 "\n", value);
    fflush(stdout);
}
