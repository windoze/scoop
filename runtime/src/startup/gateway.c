#include "../thread.h"
#include "internal.h"

__attribute__((noinline)) uint32_t scoop_startup_call_gateway(uint32_t (*gateway)(void)) {
    scoop_thread_enter_gateway(__builtin_frame_address(0));
    uint32_t status = gateway();
    scoop_thread_leave_managed();
    if (status > 1) {
        scoop_startup_fatal("gateway returned an invalid status");
    }
    return status;
}

__attribute__((noinline)) uint32_t scoop_startup_call_root(ScoopRootEntryGatewayFnV1 gateway,
                                                           int32_t argc, const char *const *argv,
                                                           int32_t *out_exit_code) {
    scoop_thread_enter_gateway(__builtin_frame_address(0));
    uint32_t status = gateway(argc, argv, out_exit_code);
    scoop_thread_leave_managed();
    if (status > 1) {
        scoop_startup_fatal("gateway returned an invalid status");
    }
    return status;
}
