#include <stdint.h>
#include <stdlib.h>

static unsigned calls;
static void verify(int value) { if (!value) abort(); }
typedef struct { uint8_t flag; uint64_t pin; uint64_t handle; } NativeHandles;
uint64_t native_handle;
NativeHandles exchange_handles(NativeHandles value, const uint64_t *pointer,
                               uint64_t (*callback)(uint64_t)) {
    verify(value.flag == 3 && value.pin == 2 && value.handle == 1);
    verify(pointer != NULL && *pointer == 1);
    verify(callback != NULL && callback(7) == 7);
    value.handle = 9;
    calls++;
    return value;
}
void verify_finished(void) {
    verify(calls == 1 && native_handle == 9);
}
