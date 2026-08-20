#include <stdint.h>
#include <stdlib.h>

static unsigned calls;
static void verify(int value) { if (!value) abort(); }
uint64_t handle_round_trip(uint64_t value) {
    verify(value == (calls == 0 ? 42 : 99));
    calls++;
    return calls == 1 ? value : value + 1;
}
void verify_finished(void) { verify(calls == 2); }
