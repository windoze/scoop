/* Stop-the-world tracing collector. Heap layout, allocation and reclamation
 * are owned by gc.c; this module owns root enumeration and graph traversal. */

#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "scoop_rt.h"
#include "../thread.h"
#include "gc_internal.h"

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
            collector_fatal("out of memory growing the mark worklist");
        }
        work = grown;
        work_cap = new_cap;
    }
    work[work_len++] = object;
}

/* The caller holds the heap lock, so object-start and mark metadata are a
 * coherent snapshot throughout tracing. */
static void mark(void *object) {
    if (scoop_gc_mark_object_locked(object)) {
        marked_count++;
        work_push(object);
    }
}

static void trace_slot(void **slot) {
    void *target = *slot;
    if (target == NULL) {
        return;
    }
    if (scoop_gc_is_object_start_locked(target)) {
        mark(target);
        return;
    }
    if (!scoop_gc_is_immortal_object_locked(target)) {
        collector_fatal(
            "managed slot points outside the heap and immortal object table");
    }
}

/* Scan one inline value as directed by the recursive descriptor in
 * scoop_rt.h. Every plain reference offset is relative to base. */
static void trace_descriptor(void *base, const uint64_t *table) {
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
            trace_descriptor(elements + index * stride, element_scan);
        }
        return;
    }
    if (table[0] == SCOOP_REFS_SEQUENCE) {
        uint64_t child_count = table[1];
        for (uint64_t index = 0; index < child_count; index++) {
            trace_descriptor(base,
                             (const uint64_t *)(uintptr_t)table[2 + index]);
        }
        return;
    }
    uint64_t count = table[0];
    for (uint64_t index = 0; index < count; index++) {
        trace_slot((void **)((char *)base + table[1 + index]));
    }
}

static void trace_object(void *object) {
    const uint64_t *refs = ((ScoopObjectHeader *)object)->td->ref_offsets;
    trace_descriptor(object, refs);
}

static void visit_root_slot(void **slot, void *context) {
    (void)context;
    trace_slot(slot);
}

static void visit_external_root(const void *object, void *context) {
    (void)context;
    trace_object((void *)object);
}

static void visit_root_region(void *base, const uint64_t *scan,
                              void *context) {
    (void)context;
    trace_descriptor(base, scan);
}

static void scan_range(const char *low, const char *high) {
    uintptr_t cursor = ((uintptr_t)low + sizeof(void *) - 1) &
                       ~(uintptr_t)(sizeof(void *) - 1);
    for (; cursor + sizeof(void *) <= (uintptr_t)high;
         cursor += sizeof(void *)) {
        void *word;
        memcpy(&word, (const void *)cursor, sizeof word);
        if (scoop_gc_is_object_start_locked(word)) {
            mark(word);
        }
    }
}

static void scan_native_roots(const ScoopThreadState *thread) {
    for (ScoopNativeRootFrame *frame = thread->native_roots; frame != NULL;
         frame = frame->previous) {
        for (uint64_t index = 0; index < frame->count; index++) {
            trace_slot(frame->slots[index]);
        }
    }
}

static void scan_caller_roots(const ScoopThreadState *thread) {
    for (ScoopCallerRootFrame *frame = thread->caller_roots; frame != NULL;
         frame = frame->previous) {
        for (uint64_t index = 0; index < frame->count; index++) {
            trace_descriptor(frame->entries[index].base,
                             frame->entries[index].scan);
        }
    }
}

static void scan_frozen_managed_segments(const ScoopThreadState *thread) {
    for (ScoopThreadTransition *transition = thread->current_transition;
         transition != NULL; transition = transition->previous) {
        uintptr_t low = transition->managed_stack_low;
        uintptr_t high = transition->managed_stack_high;
        if (low < (uintptr_t)thread->stack_low || low >= high ||
            high > (uintptr_t)thread->stack_high) {
            collector_fatal(
                "native transition contains an invalid managed stack segment");
        }
        scan_range((const char *)low, (const char *)high);
    }
}

/* M13 transition: parked managed threads publish a stable stack range.
 * M15 replaces these conservative ranges with exact stackmap slots while
 * retaining the root visitor below. */
static void scan_thread(const ScoopThreadState *thread) {
    ScoopThreadMode mode =
        atomic_load_explicit(&thread->mode, memory_order_acquire);
    if (mode == SCOOP_THREAD_PARKED || mode == SCOOP_THREAD_COLLECTOR) {
        if (thread->parked_from == SCOOP_THREAD_MANAGED) {
            uintptr_t stack_low = (uintptr_t)thread->stack_low;
            uintptr_t stack_high = (uintptr_t)thread->stack_high;
            uintptr_t parked_sp = (uintptr_t)thread->parked_sp;
            uintptr_t managed_boundary =
                (uintptr_t)thread->managed_stack_boundary;
            if (parked_sp == 0 || parked_sp < stack_low ||
                managed_boundary == 0 || managed_boundary > stack_high ||
                parked_sp >= managed_boundary) {
                collector_fatal(
                    "parked thread published an invalid stack pointer");
            }
            scan_range((const char *)&thread->register_spill,
                       (const char *)&thread->register_spill +
                           sizeof thread->register_spill);
            scan_range(thread->parked_sp, thread->managed_stack_boundary);
        } else if (thread->parked_from != SCOOP_THREAD_NATIVE_BORROWED) {
            collector_fatal("parked thread has an invalid source mode");
        }
    } else if (mode != SCOOP_THREAD_NATIVE_SAFE) {
        collector_fatal("collector observed a non-quiescent thread");
    }
    scan_frozen_managed_segments(thread);
    scan_caller_roots(thread);
    scan_native_roots(thread);
}

void scoop_rt_gc_collect(void) {
    if (!scoop_thread_begin_collection()) {
        return;
    }
    /* Heap-before-roots is the global metadata lock order. Native-safe
     * threads may still use root APIs, so roots remain locked through heap
     * reclamation to provide one coherent snapshot. */
    scoop_gc_heap_lock();
    scoop_gc_roots_lock();

    for (ScoopThreadState *thread = scoop_thread_collection_registry_head();
         thread != NULL; thread = thread->registry_next) {
        thread->allocation.cursor = NULL;
        thread->allocation.limit = NULL;
    }
    scoop_gc_heap_begin_collection_locked();
    marked_count = 0;

    ScoopGcRootVisitor root_visitor = {
        .visit_slot = visit_root_slot,
        .visit_external_object = visit_external_root,
        .visit_region = visit_root_region,
        .context = NULL,
    };
    scoop_gc_visit_roots_locked(root_visitor);
    for (ScoopThreadState *thread = scoop_thread_collection_registry_head();
         thread != NULL; thread = thread->registry_next) {
        scan_thread(thread);
    }

    while (work_len > 0) {
        trace_object(work[--work_len]);
    }

    scoop_gc_heap_finish_collection_locked(marked_count);
    scoop_gc_roots_unlock();
    scoop_gc_heap_unlock();
    scoop_thread_end_collection();
}

void scoop_rt_safepoint(void) {
    scoop_thread_poll();
}
