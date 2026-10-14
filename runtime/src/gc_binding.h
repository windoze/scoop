#ifndef SCOOP_GC_BINDING_H
#define SCOOP_GC_BINDING_H

#include <stdbool.h>
#include <stdint.h>

/* Scoop representation adapter. All visits address real slots in the existing
 * frozen view; workers do not acquire locks or copy root/metadata directories. */
typedef void (*ScoopGcSlotVisitor)(void **slot, void *context);
typedef void (*ScoopGcExternalObjectVisitor)(const void *object, void *context);
typedef void (*ScoopGcRegionVisitor)(void *base, const uint64_t *scan, void *context);

typedef struct ScoopGcRootVisitor {
    ScoopGcSlotVisitor visit_slot;
    ScoopGcExternalObjectVisitor visit_external_object;
    ScoopGcRegionVisitor visit_region;
    void *context;
} ScoopGcRootVisitor;

/* Collection holds this lock while it visits and rewrites root slots;
 * mutator root APIs use the same lock. */
void scoop_gc_roots_lock(void);
void scoop_gc_roots_unlock(void);
_Noreturn void scoop_gc_roots_fatal(const char *message);
bool scoop_gc_is_immortal_object_locked(const void *object);
bool scoop_gc_is_external_object_locked(const void *object);
/* Read-only during mark while the coordinator owns the stable heap/root view. */
bool scoop_gc_stw_is_stable_object(const void *object);
void scoop_gc_visit_roots_locked(ScoopGcRootVisitor visitor);

/* Shared exact scans. Partial heap scans use a half-open card range. */
void scoop_gc_scan_roots(ScoopGcRootVisitor visitor);
void scoop_gc_scan_descriptor(void *base, const uint64_t *scan, ScoopGcSlotVisitor visitor,
                              void *context, uintptr_t begin, uintptr_t end);

void scoop_gc_visit_pin_frames(ScoopGcSlotVisitor visitor, void *context);

#endif
