#include <stdint.h>
#include <stdlib.h>

static unsigned calls;
static void verify(int value) { if (!value) abort(); }
int8_t native_narrow(int8_t value) {
    verify(value == 17);
    calls++;
    return 18;
}
void verify_finished(void) { verify(calls == 1); }
