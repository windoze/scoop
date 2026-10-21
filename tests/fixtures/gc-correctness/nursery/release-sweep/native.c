#include <assert.h>
#include <stdint.h>

static int32_t released;
static unsigned seen[512];

void m58_released(int32_t value) {
    assert(value >= 0 && value < 512);
    assert(++seen[value] == 1);
    released++;
}

int32_t m58_release_count(void) { return released; }
