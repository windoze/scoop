#ifndef SCOOP_RT_CALLBACK_H
#define SCOOP_RT_CALLBACK_H

#include <stdint.h>

typedef struct ScoopCallbackShutdownCounts {
    uint64_t active;
    uint64_t owned_tokens;
} ScoopCallbackShutdownCounts;

ScoopCallbackShutdownCounts scoop_callback_prepare_shutdown(void);

#endif
