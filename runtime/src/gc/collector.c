/* STW phases: parallel mark/update, serial plan/copy and reclaim. */
#include <stdint.h>

#include "../gc_binding/object.h"
#include "gc_internal.h"
#include "heap_internal.h"

typedef struct ScoopGcVisitContext {
    bool minor;
#if SCOOP_GC_TRACK_RELOCATION
    bool verify;
#endif
    bool remember_survivors;
} ScoopGcVisitContext;

static bool remains_young(const ScoopGcBlockMeta *block) {
    return block->generation == SCOOP_GC_YOUNG && block->retained_young;
}

static inline __attribute__((always_inline)) void
relocate_slot(void **slot, void *raw_context, bool remember_survivors) {
    ScoopGcVisitContext *context = raw_context;
    void *target = *slot;
    if (target == NULL) {
        return;
    }
    ScoopGcBlockMeta *block;
    size_t word;
    if (!scoop_heap_reference_meta(target, &block, &word)) {
#if SCOOP_ENABLE_GC_STRESS || (defined(SCOOP_GC_VERIFY_HEAP) && SCOOP_GC_VERIFY_HEAP)
        if (!scoop_gc_is_immortal_object_locked(target)) {
            heap_fatal("managed slot points outside the current heap and stable object tables");
        }
#endif
        return;
    }
    if (context->minor && block->generation == SCOOP_GC_OLD) {
        return;
    }
    void *current = scoop_heap_forward_object(target, block, word);
    if (current != target) {
#if SCOOP_GC_TRACK_RELOCATION
        if (context->verify) {
            heap_fatal("managed slot still points to a forwarded old object");
        }
#endif
        *slot = current;
    }
    if (remember_survivors) {
        const ScoopGcBlockMeta *target_block = current == target ? block : pointer_block(current);
        if (remains_young(target_block)) {
            ScoopGcBlockMeta *owner = pointer_block(slot);
            if (!remains_young(owner)) {
                ScoopGcRegion *region = owner->region;
                size_t card = ((uintptr_t)slot - region->base) >> GC_CARD_SHIFT;
                __atomic_store_n(&region->cards[card], 2, __ATOMIC_RELAXED);
                scoop_runtime_record_dirty_card_group(region, card);
            }
        }
    }
}

static void visit_managed_slot(void **slot, void *context) { relocate_slot(slot, context, false); }

static void visit_survivor_slot(void **slot, void *context) { relocate_slot(slot, context, true); }

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
    ScoopGcVisitContext *visit = context;
    ScoopGcSlotVisitor slot = visit->remember_survivors ? visit_survivor_slot : visit_managed_slot;
    scoop_gc_scan_descriptor(object, scan, slot, context, begin, end);
}

static void relocate_live_object(void **entry, void *context) {
    void *object = *entry;
    const uint64_t *scan = scoop_gc_object_scan(object);
    if (scan == NULL) {
        return;
    }
#if SCOOP_GC_TRACK_RELOCATION
    scoop_heap_claim_current_object(object);
#endif
    ScoopGcVisitContext *visit = context;
    ScoopGcSlotVisitor slot = visit->remember_survivors ? visit_survivor_slot : visit_managed_slot;
    scoop_gc_scan_descriptor(object, scan, slot, context, 0, UINTPTR_MAX);
}

#if SCOOP_GC_TRACK_RELOCATION
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
#endif

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
    ScoopGcSurvivors survivors;
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
        bool planned = scoop_gc_heap_plan_moving_locked(minor, &survivors);
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

    if (moved_objects != 0 || survivors.objects != 0) {
        uint64_t phase = scoop_gc_monotonic_ns();
        ScoopGcVisitContext relocate = {.minor = minor};
        if (moved_objects != 0) {
            scan_roots(&relocate);
        }
        relocate.remember_survivors = survivors.objects != 0;
        if (minor) {
            scoop_gc_scan_remembered(visit_dirty_object, &relocate, false);
        }
        ScoopGcSlotVisitor slot = survivors.objects != 0 ? visit_survivor_slot : visit_managed_slot;
        scoop_gc_update_live(slot, relocate_live_object, &relocate);
        metrics->update_ns += scoop_gc_monotonic_ns() - phase;
    }
    metrics->minor_collections += minor;
    metrics->full_collections += !minor;

#if SCOOP_GC_TRACK_RELOCATION
    bool verify_heap = scoop_gc_stress_move_enabled();
#if defined(SCOOP_GC_VERIFY_HEAP) && SCOOP_GC_VERIFY_HEAP
    verify_heap = true;
#endif
    if (verify_heap) {
        ScoopGcVisitContext verify = {.minor = minor, .verify = true};
        scan_roots(&verify);
        scoop_gc_visit_current_objects_locked(verify_heap_object, &verify);
    }
#endif
#if SCOOP_ENABLE_GC_STRESS
    if (scoop_gc_stress_move_enabled()) {
        scoop_gc_heap_verify_stress_moved_locked();
    }
#endif
    scoop_gc_mark_dispose();
    uint64_t phase = scoop_gc_monotonic_ns();
    uint64_t returned = metrics->vm_return_ns;
    scoop_gc_heap_finish_collection_locked(marked_count, minor, &survivors);
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
