/* STW phases: parallel mark, serial plan/copy, retained-set update and reclaim. */
#include <stdint.h>

#include "../managed_entries.h"
#include "../thread.h"
#include "gc_internal.h"
#include "heap_internal.h"

typedef struct ScoopGcVisitContext {
    bool minor;
    bool verify;
} ScoopGcVisitContext;

static void visit_managed_slot(void **slot, void *raw_context) {
    ScoopGcVisitContext *context = raw_context;
    void *target = *slot;
    if (target == NULL) {
        return;
    }
    if (!scoop_gc_is_object_start_locked(target)) {
        if (!scoop_gc_is_immortal_object_locked(target) &&
            !scoop_gc_is_external_object_locked(target)) {
            heap_fatal("managed slot points outside the current heap and stable object tables");
        }
        return;
    }
    if (context->minor && !scoop_gc_is_young_object_locked(target)) {
        return;
    }
    if (context->verify) {
        if (scoop_gc_is_forwarded_old_locked(target)) {
            heap_fatal("managed slot still points to a forwarded old object");
        }
        if (!scoop_gc_is_current_live_object_locked(target)) {
            heap_fatal("managed slot points to a dead heap object");
        }
    } else {
        *slot = scoop_gc_forward_object_locked(target);
    }
}

static void visit_object(void *object, void *context) {
    const ScoopTypeDescriptor *td = ((const ScoopObjectHeader *)object)->td;
    scoop_gc_scan_descriptor(object, td->object_scan, visit_managed_slot, context, 0, UINTPTR_MAX);
}

static void visit_external_root(const void *object, void *context) {
    visit_object((void *)object, context);
}

static void visit_root_region(void *base, const uint64_t *scan, void *context) {
    scoop_gc_scan_descriptor(base, scan, visit_managed_slot, context, 0, UINTPTR_MAX);
}

static void scan_roots(ScoopGcVisitContext *context) {
    scoop_gc_scan_roots((ScoopGcRootVisitor){
        .visit_slot = visit_managed_slot,
        .visit_external_object = visit_external_root,
        .visit_region = visit_root_region,
        .context = context,
    });
}

static void visit_dirty_object(void *object, uintptr_t begin, uintptr_t end, void *context) {
    const ScoopTypeDescriptor *td = ((const ScoopObjectHeader *)object)->td;
    scoop_gc_scan_descriptor(object, td->object_scan, visit_managed_slot, context, begin, end);
}

static void relocate_live_object(void *object, void *context) {
    void *current = scoop_gc_forward_object_locked(object);
    if (!scoop_gc_claim_object_scan_locked(current)) {
        heap_fatal("marked object appeared twice in the retained live set");
    }
    visit_object(current, context);
}

static void verify_heap_object(void *object, void *context) {
    if (!scoop_gc_object_was_scanned_locked(object)) {
        heap_fatal("live object was not visited by retained-set relocation");
    }
    visit_object(object, context);
}

static void record_pause(uint64_t elapsed, bool minor) {
    ScoopGcMetrics *metrics = &scoop_gc_heap_state.metrics;
    metrics->pause_ns += elapsed;
    if (elapsed > metrics->maximum_pause_ns) {
        metrics->maximum_pause_ns = elapsed;
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
}

static bool collect(bool minor) {
    uint64_t requested = scoop_gc_monotonic_ns();
    if (!scoop_thread_begin_collection()) {
        return false;
    }
    uint64_t started = scoop_gc_monotonic_ns();
    scoop_gc_heap_lock();
    scoop_gc_roots_lock();
    ScoopGcMetrics *metrics = &scoop_gc_heap_state.metrics;
    metrics->stop_wait_ns += started - requested;
    scoop_gc_set_pin_frames_locked(true);
    for (ScoopThreadState *thread = scoop_thread_collection_registry_head(); thread != NULL;
         thread = thread->registry_next) {
        thread->allocation.cursor = NULL;
        thread->allocation.limit = NULL;
        thread->allocation_block = NULL;
    }
    uint64_t marked_count;
    for (;;) {
        size_t block_count = scoop_gc_heap_begin_collection_locked(minor);
        scoop_gc_mark_begin(minor, block_count);
        uint64_t phase = scoop_gc_monotonic_ns();
        scoop_gc_mark_roots();
        metrics->root_scan_ns += scoop_gc_monotonic_ns() - phase;
        if (minor) {
            phase = scoop_gc_monotonic_ns();
            scoop_gc_mark_remembered();
            metrics->remembered_scan_ns += scoop_gc_monotonic_ns() - phase;
        }
        phase = scoop_gc_monotonic_ns();
        marked_count = scoop_gc_mark_finish(minor);
        metrics->mark_ns += scoop_gc_monotonic_ns() - phase;
        phase = scoop_gc_monotonic_ns();
        uint64_t copied = metrics->copy_ns;
        bool planned = scoop_gc_heap_plan_moving_locked(minor);
        metrics->plan_ns += scoop_gc_monotonic_ns() - phase - (metrics->copy_ns - copied);
        if (planned || !minor) {
            break;
        }
        /* No copies or forwarding were published by a failed reservation. */
        scoop_gc_mark_dispose();
        metrics->promotion_fallbacks++;
        collection_active = false;
        minor = false;
    }

    uint64_t phase = scoop_gc_monotonic_ns();
    ScoopGcVisitContext relocate = {.minor = minor};
    scan_roots(&relocate);
    if (minor) {
        scoop_gc_scan_remembered(visit_dirty_object, &relocate, false);
    }
    scoop_gc_mark_visit_live(relocate_live_object, &relocate);
    metrics->update_ns += scoop_gc_monotonic_ns() - phase;
    metrics->minor_collections += minor;
    metrics->full_collections += !minor;

    bool verify_heap = scoop_gc_stress_move_enabled();
#ifdef SCOOP_VERIFY_METADATA
    verify_heap = true;
#endif
    if (verify_heap) {
        ScoopGcVisitContext verify = {.minor = minor, .verify = true};
        scan_roots(&verify);
        scoop_gc_visit_current_objects_locked(verify_heap_object, &verify);
    }
    if (scoop_gc_stress_move_enabled()) {
        scoop_gc_heap_verify_stress_moved_locked();
    }
    scoop_gc_mark_dispose();
    phase = scoop_gc_monotonic_ns();
    uint64_t returned = metrics->vm_return_ns;
    scoop_gc_heap_finish_collection_locked(marked_count, minor);
    metrics->reclaim_ns += scoop_gc_monotonic_ns() - phase - (metrics->vm_return_ns - returned);
    scoop_gc_set_pin_frames_locked(false);
    record_pause(scoop_gc_monotonic_ns() - started, minor);
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
