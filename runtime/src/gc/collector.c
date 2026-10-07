/* Stop-the-world exact moving collector.
 *
 * The collection is a closed phase sequence: mark, plan, relocate/update,
 * verify, then retire. Every managed pointer source is reduced to the same
 * writable slot visitor; heap storage details remain in heap.c. */
#define _POSIX_C_SOURCE 200809L
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

#include "../managed_entries.h"
#include "../thread.h"
#include "../value_shape.h"
#include "gc_internal.h"
#include "heap_internal.h"
#include "scoop_rt.h"

typedef enum ScoopGcVisitMode {
    SCOOP_GC_VISIT_MARK,
    SCOOP_GC_VISIT_RELOCATE,
    SCOOP_GC_VISIT_VERIFY,
} ScoopGcVisitMode;

typedef struct ScoopGcVisitContext {
    ScoopGcVisitMode mode;
    bool minor;
    bool roots;
    bool remembered;
    const void *last_dirty_object;
} ScoopGcVisitContext;

static void **work;
static size_t work_len;
static size_t work_cap;
static size_t work_scanned;
static uint64_t marked_count;

static _Noreturn void collector_fatal(const char *message) {
    fprintf(stderr, "scoop gc: %s\n", message);
    abort();
}

static void work_push(void *object) {
    if (work_len == work_cap) {
        size_t new_cap = work_cap == 0 ? 256 : work_cap * 2;
        void **grown = realloc(work, new_cap * sizeof *grown);
        if (grown == NULL) {
            collector_fatal("out of memory growing the trace worklist");
        }
        work = grown;
        work_cap = new_cap;
    }
    work[work_len++] = object;
}

static bool stable_object(const void *object) {
    return scoop_gc_is_immortal_object_locked(object) || scoop_gc_is_external_object_locked(object);
}

static void visit_managed_slot(void **slot, void *raw_context) {
    ScoopGcVisitContext *context = raw_context;
    if (context->mode == SCOOP_GC_VISIT_MARK) {
        if (context->roots) {
            scoop_gc_heap_state.metrics.root_slots++;
        } else if (context->remembered) {
            scoop_gc_heap_state.metrics.old_reference_slots++;
        }
    }
    void *target = *slot;
    if (target == NULL) {
        return;
    }
    if (!scoop_gc_is_object_start_locked(target)) {
        if (!stable_object(target)) {
            collector_fatal("managed slot points outside the current heap and stable "
                            "object tables");
        }
        return;
    }

    if (context->minor && !scoop_gc_is_young_object_locked(target)) {
        return;
    }
    switch (context->mode) {
    case SCOOP_GC_VISIT_MARK:
        if (scoop_gc_mark_object_locked(target)) {
            marked_count++;
            work_push(target);
        }
        return;
    case SCOOP_GC_VISIT_RELOCATE: {
        void *current = scoop_gc_forward_object_locked(target);
        *slot = current;
        if (!context->minor && scoop_gc_claim_object_scan_locked(current)) {
            work_push(current);
        }
        return;
    }
    case SCOOP_GC_VISIT_VERIFY:
        if (scoop_gc_is_forwarded_old_locked(target)) {
            collector_fatal("managed slot still points to a forwarded old object");
        }
        if (!scoop_gc_is_current_live_object_locked(target)) {
            collector_fatal("managed slot points to a dead heap object");
        }
        return;
    }
    collector_fatal("invalid managed-slot visitor mode");
}

static void visit_object(void *object, ScoopGcVisitContext *context) {
    const ScoopTypeDescriptor *td = ((const ScoopObjectHeader *)object)->td;
    if (td == NULL) {
        collector_fatal("managed object has no TypeDescriptor");
    }
    if (context->mode == SCOOP_GC_VISIT_MARK && scoop_gc_is_object_start_locked(object)) {
        scoop_shape_validate_object(object, scoop_gc_object_size_locked(object));
    }
    if (context->mode == SCOOP_GC_VISIT_MARK) {
        scoop_gc_heap_state.metrics.traced_objects++;
    }
    scoop_gc_scan_descriptor(object, td->object_scan, visit_managed_slot, context, 0, UINTPTR_MAX);
}

static void visit_external_root(const void *object, void *raw_context) {
    visit_object((void *)object, raw_context);
}

static void visit_root_region(void *base, const uint64_t *scan, void *raw_context) {
    scoop_gc_scan_descriptor(base, scan, visit_managed_slot, raw_context, 0, UINTPTR_MAX);
}

static ScoopGcRootVisitor root_visitor(ScoopGcVisitContext *context) {
    return (ScoopGcRootVisitor){
        .visit_slot = visit_managed_slot,
        .visit_external_object = visit_external_root,
        .visit_region = visit_root_region,
        .context = context,
    };
}

static void drain_work(ScoopGcVisitContext *context) {
    while (work_scanned < work_len) {
        visit_object(work[work_scanned++], context);
    }
}

static void verify_heap_object(void *object, void *raw_context) {
    if (!scoop_gc_object_was_scanned_locked(object)) {
        collector_fatal("live object was not reached by relocation tracing");
    }
    visit_object(object, raw_context);
}

static void scan_roots(ScoopGcVisitContext *context) {
    context->roots = true;
    scoop_gc_scan_roots(root_visitor(context));
    context->roots = false;
}

static void visit_dirty_object(void *object, uintptr_t begin, uintptr_t end, void *raw_context) {
    ScoopGcVisitContext *context = raw_context;
    const ScoopTypeDescriptor *td = ((ScoopObjectHeader *)object)->td;
    if (context->mode == SCOOP_GC_VISIT_MARK && context->last_dirty_object != object) {
        scoop_shape_validate_object(object, scoop_gc_object_size_locked(object));
        context->last_dirty_object = object;
    }
    context->remembered = true;
    scoop_gc_scan_descriptor(object, td->object_scan, visit_managed_slot, context, begin, end);
    context->remembered = false;
}

static uint64_t monotonic_ns(void) {
    struct timespec time;
    if (clock_gettime(CLOCK_MONOTONIC, &time) != 0) {
        collector_fatal("cannot measure collection time");
    }
    return (uint64_t)time.tv_sec * UINT64_C(1000000000) + (uint64_t)time.tv_nsec;
}

static bool collect(bool minor) {
    if (!scoop_thread_begin_collection()) {
        return false;
    }
    uint64_t started = monotonic_ns();
    scoop_gc_heap_lock();
    scoop_gc_roots_lock();
    for (ScoopThreadState *thread = scoop_thread_collection_registry_head(); thread != NULL;
         thread = thread->registry_next) {
        thread->allocation.cursor = NULL;
        thread->allocation.limit = NULL;
    }
    if (work_len != 0) {
        collector_fatal("trace worklist was not drained by the prior collection");
    }
    scoop_gc_heap_begin_collection_locked(minor);
    for (;;) {
        marked_count = 0;
        work_len = work_scanned = 0;
        ScoopGcVisitContext mark = {.mode = SCOOP_GC_VISIT_MARK, .minor = minor};
        scan_roots(&mark);
        if (minor) {
            scoop_gc_scan_remembered(visit_dirty_object, &mark, true);
        }
        drain_work(&mark);
        if (scoop_gc_heap_plan_moving_locked(minor) || !minor) {
            break;
        }
        // A failed reservation has not copied objects or published forwarding.
        scoop_gc_heap_state.metrics.promotion_fallbacks++;
        collection_active = false;
        minor = false;
        scoop_gc_heap_begin_collection_locked(false);
    }

    ScoopGcVisitContext relocate = {.mode = SCOOP_GC_VISIT_RELOCATE, .minor = minor};
    if (minor) {
        scan_roots(&relocate);
        scoop_gc_scan_remembered(visit_dirty_object, &relocate, false);
        for (size_t index = 0; index < work_len; index++) {
            void *object = scoop_gc_forward_object_locked(work[index]);
            (void)scoop_gc_claim_object_scan_locked(object);
            visit_object(object, &relocate);
        }
        scoop_gc_heap_state.metrics.minor_collections++;
    } else {
        work_len = work_scanned = 0;
        scan_roots(&relocate);
        drain_work(&relocate);
        scoop_gc_heap_state.metrics.full_collections++;
    }

    bool verify_heap = scoop_gc_stress_move_enabled();
#ifdef SCOOP_VERIFY_METADATA
    verify_heap = true;
#endif
    if (verify_heap) {
        ScoopGcVisitContext verify = {.mode = SCOOP_GC_VISIT_VERIFY, .minor = minor};
        scan_roots(&verify);
        scoop_gc_visit_current_objects_locked(verify_heap_object, &verify);
    }
    if (scoop_gc_stress_move_enabled()) {
        scoop_gc_heap_verify_stress_moved_locked();
    }
    work_len = work_scanned = 0;
    scoop_gc_heap_finish_collection_locked(marked_count, minor);
    uint64_t elapsed = monotonic_ns() - started;
    scoop_gc_heap_state.metrics.pause_ns += elapsed;
    if (elapsed > scoop_gc_heap_state.metrics.maximum_pause_ns) {
        scoop_gc_heap_state.metrics.maximum_pause_ns = elapsed;
    }
    if (minor) {
        scoop_gc_heap_state.minor_pause_ns += elapsed;
    } else {
        scoop_gc_heap_state.full_pause_ns += elapsed;
    }
    static const uint64_t bounds[] = {10000, 50000, 100000, 500000, 1000000, 5000000, 10000000};
    size_t bucket = 0;
    while (bucket < 7 && elapsed > bounds[bucket]) {
        bucket++;
    }
    scoop_gc_heap_state.pause_buckets[bucket]++;
    scoop_gc_roots_unlock();
    scoop_gc_heap_unlock();
    scoop_thread_end_collection();
    return true;
}

bool scoop_gc_collect_internal(void) { return collect(false); }

void scoop_gc_collect_minor_internal(void) {
    collect(!scoop_gc_stress_move_enabled() && !scoop_gc_heap_state.full_only);
}

void scoop_rt_gc_collect_impl(uintptr_t return_pc, uintptr_t stack_pointer,
                              uintptr_t frame_pointer) {
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, return_pc, stack_pointer, frame_pointer);
    scoop_gc_collect_internal();
    scoop_thread_pop_managed_anchor(&anchor);
}

void scoop_runtime_gc_collect(void) {
    scoop_thread_native_borrowed_entry();
    scoop_gc_collect_internal();
}

void scoop_rt_safepoint_impl(uintptr_t return_pc, uintptr_t stack_pointer,
                             uintptr_t frame_pointer) {
    ScoopManagedAnchor anchor;
    scoop_thread_push_safepoint_anchor(&anchor, return_pc, stack_pointer, frame_pointer);
    scoop_thread_poll();
    scoop_thread_pop_managed_anchor(&anchor);
}
