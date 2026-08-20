#include <stdio.h>
#include <stdlib.h>

#include "platform.h"

const char *scoop_platform_error_message(ScoopPlatformErrorCode code) {
    switch (code) {
    case SCOOP_PLATFORM_OK:
        return "no error";
    case SCOOP_PLATFORM_METADATA_UNAVAILABLE:
        return "loaded image metadata is unavailable";
    case SCOOP_PLATFORM_INVALID_STACK_BOUNDS:
        return "OS thread stack bounds are invalid";
    case SCOOP_PLATFORM_INVALID_ANCHOR:
        return "managed entry anchor is invalid";
    case SCOOP_PLATFORM_INVALID_FRAME:
        return "managed frame chain is invalid";
    case SCOOP_PLATFORM_UNSUPPORTED_ROOT_LOCATION:
        return "stack map root location violates the target profile";
    case SCOOP_PLATFORM_ROOT_OUTSIDE_FRAME:
        return "stack map root lies outside its managed frame";
    case SCOOP_PLATFORM_VM_OPERATION_FAILED:
        return "virtual-memory operation failed";
    }
    return "unknown platform error";
}

ScoopPlatformStackBounds scoop_platform_stack_bounds(void) {
    ScoopPlatformStackBounds bounds;
    ScoopPlatformError error = {0};
    if (!scoop_platform_bundle()->thread_vm->stack_bounds(&bounds, &error)) {
        fprintf(stderr, "scoop runtime: %s\n",
                scoop_platform_error_message(error.code));
        abort();
    }
    return bounds;
}
