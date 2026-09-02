/* Stop-the-world exact moving collector.
 *
 * The collection is a closed phase sequence: mark, plan, relocate/update,
 * verify, then retire. Every managed pointer source is reduced to the same
 * writable slot visitor; heap storage details remain in heap.c. */
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#include "scoop_rt.h"
#include "../managed_entries.h"
#include "../thread.h"
#include "gc_internal.h"

typedef enum ScoopGcVisitMode {
    SCOOP_GC_VISIT_MARK,
    SCOOP_GC_VISIT_RELOCATE,
    SCOOP_GC_VISIT_VERIFY,
} ScoopGcVisitMode;

typedef struct ScoopGcVisitContext {
    ScoopGcVisitMode mode;
} ScoopGcVisitContext;

static void **work;
static size_t work_len;
static size_t work_cap;
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
    return scoop_gc_is_immortal_object_locked(object) ||
           scoop_gc_is_external_object_locked(object);
}

static void visit_managed_slot(void **slot, void *raw_context) {
    ScoopGcVisitContext *context = raw_context;
    void *target = *slot;
    if (target == NULL) {
        return;
    }
    if (!scoop_gc_is_object_start_locked(target)) {
        if (!stable_object(target)) {
            collector_fatal(
                "managed slot points outside the current heap and stable object tables");
        }
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
        if (scoop_gc_claim_object_scan_locked(current)) {
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

/* Every descriptor is complete and relative to base. Array descriptors are
 * object scans: their length and inline element storage follow the runtime
 * Array layout at offsets 16 and 24. */
static void visit_descriptor(void *base, const uint64_t *table,
                             ScoopGcVisitContext *context) {
    if (table == NULL) {
        return;
    }
    if (table[0] == SCOOP_REFS_ARRAY) {
        uint64_t stride = table[1];
        const uint64_t *element_scan =
            (const uint64_t *)(uintptr_t)table[2];
        uint64_t count = *(const uint64_t *)((char *)base + 16);
        char *elements = (char *)base + 24;
        for (uint64_t index = 0; index < count; index++) {
            visit_descriptor(elements + index * stride, element_scan,
                             context);
        }
        return;
    }
    if (table[0] == SCOOP_REFS_SEQUENCE) {
        uint64_t child_count = table[1];
        for (uint64_t index = 0; index < child_count; index++) {
            visit_descriptor(
                base, (const uint64_t *)(uintptr_t)table[2 + index],
                context);
        }
        return;
    }
    uint64_t count = table[0];
    for (uint64_t index = 0; index < count; index++) {
        visit_managed_slot((void **)((char *)base + table[1 + index]),
                           context);
    }
}

static void visit_object(void *object, ScoopGcVisitContext *context) {
    const ScoopTypeDescriptor *td =
        ((const ScoopObjectHeader *)object)->td;
    if (td == NULL) {
        collector_fatal("managed object has no TypeDescriptor");
    }
    visit_descriptor(object, td->ref_offsets, context);
}

static void visit_external_root(const void *object, void *raw_context) {
    visit_object((void *)object, raw_context);
}

static void visit_root_region(void *base, const uint64_t *scan,
                              void *raw_context) {
    visit_descriptor(base, scan, raw_context);
}

static ScoopGcRootVisitor root_visitor(ScoopGcVisitContext *context) {
    return (ScoopGcRootVisitor){
        .visit_slot = visit_managed_slot,
        .visit_external_object = visit_external_root,
        .visit_region = visit_root_region,
        .context = context,
    };
}

static void scan_native_roots(const ScoopThreadState *thread,
                              ScoopGcVisitContext *context) {
    for (ScoopNativeRootFrame *frame = thread->native_roots; frame != NULL;
         frame = frame->previous) {
        for (uint64_t index = 0; index < frame->count; index++) {
            visit_managed_slot(frame->slots[index], context);
        }
    }
}

static void scan_native_region_roots(const ScoopThreadState *thread,
                                     ScoopGcVisitContext *context) {
    for (ScoopNativeRegionRootFrame *frame = thread->native_region_roots;
         frame != NULL; frame = frame->previous) {
        for (uint64_t index = 0; index < frame->count; index++) {
            visit_descriptor(frame->entries[index].base,
                             frame->entries[index].scan, context);
        }
    }
}

static void scan_caller_roots(const ScoopThreadState *thread,
                              ScoopGcVisitContext *context) {
    for (ScoopCallerRootFrame *frame = thread->caller_roots; frame != NULL;
         frame = frame->previous) {
        for (uint64_t index = 0; index < frame->count; index++) {
            visit_descriptor(frame->entries[index].base,
                             frame->entries[index].scan, context);
        }
    }
}

static void scan_compiler_roots(const ScoopThreadState *thread,
                                ScoopGcVisitContext *context) {
    for (ScoopCompilerRootFrame *frame = thread->compiler_roots; frame != NULL;
         frame = frame->previous) {
        for (uint64_t index = 0; index < frame->count; index++) {
            visit_descriptor(frame->entries[index].base,
                             frame->entries[index].scan, context);
        }
    }
}

static void scan_thread(const ScoopThreadState *thread,
                        ScoopGcVisitContext *context) {
    ScoopThreadMode mode =
        atomic_load_explicit(&thread->mode, memory_order_acquire);
    if (mode == SCOOP_THREAD_PARKED || mode == SCOOP_THREAD_COLLECTOR) {
        if (thread->parked_from == SCOOP_THREAD_MANAGED) {
            ScoopGcRootVisitor visitor = root_visitor(context);
            scoop_gc_visit_managed_stack(thread, visitor);
        } else if (thread->parked_from != SCOOP_THREAD_NATIVE_BORROWED) {
            collector_fatal("parked thread has an invalid source mode");
        }
    } else if (mode != SCOOP_THREAD_NATIVE_SAFE) {
        collector_fatal("collector observed a non-quiescent thread");
    }
    scan_caller_roots(thread, context);
    scan_compiler_roots(thread, context);
    scan_native_region_roots(thread, context);
    scan_native_roots(thread, context);
}

static void scan_all_roots(ScoopGcVisitContext *context) {
    ScoopGcRootVisitor visitor = root_visitor(context);
    scoop_gc_visit_roots_locked(visitor);
    for (ScoopThreadState *thread = scoop_thread_collection_registry_head();
         thread != NULL; thread = thread->registry_next) {
        scan_thread(thread, context);
    }
}

static void drain_work(ScoopGcVisitContext *context) {
    while (work_len > 0) {
        visit_object(work[--work_len], context);
    }
}

static void verify_heap_object(void *object, void *raw_context) {
    if (!scoop_gc_object_was_scanned_locked(object)) {
        collector_fatal("live object was not reached by relocation tracing");
    }
    visit_object(object, raw_context);
}

void scoop_gc_collect_internal(void) {
    if (!scoop_thread_begin_collection()) {
        return;
    }
    scoop_gc_heap_lock();
    scoop_gc_roots_lock();

    for (ScoopThreadState *thread = scoop_thread_collection_registry_head();
         thread != NULL; thread = thread->registry_next) {
        thread->allocation.cursor = NULL;
        thread->allocation.limit = NULL;
    }
    if (work_len != 0) {
        collector_fatal("trace worklist was not drained by the prior collection");
    }
    scoop_gc_heap_begin_collection_locked();
    marked_count = 0;

    ScoopGcVisitContext mark = {.mode = SCOOP_GC_VISIT_MARK};
    scan_all_roots(&mark);
    drain_work(&mark);

    scoop_gc_heap_plan_moving_locked();

    ScoopGcVisitContext relocate = {.mode = SCOOP_GC_VISIT_RELOCATE};
    scan_all_roots(&relocate);
    drain_work(&relocate);

    ScoopGcVisitContext verify = {.mode = SCOOP_GC_VISIT_VERIFY};
    scan_all_roots(&verify);
    scoop_gc_visit_current_objects_locked(verify_heap_object, &verify);

    scoop_gc_heap_finish_collection_locked(marked_count);
    scoop_gc_roots_unlock();
    scoop_gc_heap_unlock();
    scoop_thread_end_collection();
}

void scoop_rt_gc_collect_impl(uintptr_t return_pc, uintptr_t stack_pointer,
                              uintptr_t frame_pointer) {
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, return_pc, stack_pointer,
                                     frame_pointer);
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
    scoop_thread_push_managed_anchor(&anchor, return_pc, stack_pointer,
                                     frame_pointer);
    scoop_thread_poll();
    scoop_thread_pop_managed_anchor(&anchor);
}
