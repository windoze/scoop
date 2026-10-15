/* STW phases: parallel mark, serial plan/copy, retained-set update and reclaim. */
#include <stdint.h>

#include "../gc_binding/object.h"
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
    ScoopGcBlockMeta *block;
    size_t word;
    if (!object_meta(target, &block, &word)) {
        if (!scoop_gc_is_immortal_object_locked(target)) {
            heap_fatal("managed slot points outside the current heap and stable object tables");
        }
        return;
    }
    if (context->minor && block->generation == SCOOP_GC_OLD) {
        return;
    }
    void *current = scoop_heap_forward_object(target, block, word);
    if (current != target) {
        if (context->verify) {
            heap_fatal("managed slot still points to a forwarded old object");
        }
        *slot = current;
    }
}

static void visit_root_region(void *base, const uint64_t *scan, void *context) {
    scoop_gc_scan_descriptor(base, scan, visit_managed_slot, context, 0, UINTPTR_MAX);
}

static void scan_roots(ScoopGcVisitContext *context) {
    scoop_gc_scan_roots((ScoopGcRootVisitor){
        .visit_slot = visit_managed_slot,
        .visit_region = visit_root_region,
        .context = context,
    });
}

static void visit_dirty_object(void *object, uintptr_t begin, uintptr_t end, void *context) {
    const uint64_t *scan = scoop_gc_object_scan(object);
    scoop_gc_scan_descriptor(object, scan, visit_managed_slot, context, begin, end);
}

static void relocate_live_object(void *object, void *context) {
    const uint64_t *scan = scoop_gc_object_scan(object);
    if (scan == NULL) {
        return;
    }
    void *current = scoop_heap_claim_relocated_object(object);
    scoop_gc_scan_descriptor(current, scan, visit_managed_slot, context, 0, UINTPTR_MAX);
}

static void verify_heap_object(void *object, void *context) {
    const uint64_t *scan = scoop_gc_object_scan(object);
    if (scan == NULL) {
        return;
    }
    if (moved_objects != 0 && !scoop_gc_object_was_scanned_locked(object)) {
        heap_fatal("live object was not visited by retained-set relocation");
    }
    scoop_gc_scan_descriptor(object, scan, visit_managed_slot, context, 0, UINTPTR_MAX);
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
    if (!scoop_gc_stop_world()) {
        return false;
    }
    uint64_t started = scoop_gc_monotonic_ns();
    scoop_gc_heap_lock();
    scoop_gc_roots_lock();
    ScoopGcMetrics *metrics = &scoop_gc_heap_state.metrics;
    metrics->stop_wait_ns += started - requested;
    scoop_gc_set_pin_frames_locked(true);
    scoop_gc_retire_allocations();
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

    if (moved_objects != 0) {
        uint64_t phase = scoop_gc_monotonic_ns();
        ScoopGcVisitContext relocate = {.minor = minor};
        scan_roots(&relocate);
        if (minor) {
            scoop_gc_scan_remembered(visit_dirty_object, &relocate, false);
        }
        scoop_gc_mark_visit_live(relocate_live_object, &relocate);
        metrics->update_ns += scoop_gc_monotonic_ns() - phase;
    }
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
    uint64_t phase = scoop_gc_monotonic_ns();
    uint64_t returned = metrics->vm_return_ns;
    scoop_gc_heap_finish_collection_locked(marked_count, minor);
    metrics->reclaim_ns += scoop_gc_monotonic_ns() - phase - (metrics->vm_return_ns - returned);
    scoop_gc_set_pin_frames_locked(false);
    record_pause(scoop_gc_monotonic_ns() - started, minor);
    scoop_gc_roots_unlock();
    scoop_gc_heap_unlock();
    scoop_gc_resume_world();
    return true;
}

bool scoop_gc_collect_internal(void) { return collect(false); }

void scoop_gc_collect_minor_internal(void) {
    collect(!scoop_gc_stress_move_enabled() && !scoop_gc_heap_state.full_only);
}
