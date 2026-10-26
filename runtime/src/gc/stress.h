#ifndef SCOOP_GC_STRESS_H
#define SCOOP_GC_STRESS_H

/* Stress support is a runtime build capability, independent of optimization. */
#ifndef SCOOP_ENABLE_GC_STRESS
#define SCOOP_ENABLE_GC_STRESS 0
#endif

/* Retained-set coverage is diagnostic state, not part of live-object identity. */
#if SCOOP_ENABLE_GC_STRESS || (defined(SCOOP_GC_VERIFY_HEAP) && SCOOP_GC_VERIFY_HEAP)
#define SCOOP_GC_TRACK_RELOCATION 1
#else
#define SCOOP_GC_TRACK_RELOCATION 0
#endif

#endif
