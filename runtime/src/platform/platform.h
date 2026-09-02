#ifndef SCOOP_PLATFORM_H
#define SCOOP_PLATFORM_H

/* Platform-neutral runtime capabilities. Generic runtime modules include
 * only this header; target-specific headers stay below platform/. */

typedef struct ScoopPlatformStackBounds {
    const char *low;
    const char *high;
} ScoopPlatformStackBounds;

/* Return the complete stack range owned by the calling OS thread. Platform
 * implementations terminate the process if the range cannot be obtained. */
ScoopPlatformStackBounds scoop_platform_stack_bounds(void);

#endif
