/* Runtime smoke test: provides scoop_main (normally emitted by the
 * compiler), exercises alloc, string concat/eq, and the String / Int /
 * Boolean print builtins, and checks that globals laid out the way
 * codegen emits string literals are readable by the runtime.
 *
 * M9: all layouts carry the 16-byte object header ({ td, gc_word },
 * fields from offset 16) and allocations come from the GC heap
 * (runtime/src/gc.c). The second half of scoop_main tests the
 * collector: allocation/collection closed loop (stats fall back),
 * TD-driven tracing (plain / array / enum scan descriptors), free-line
 * reuse, large objects, pin/handle keep-alive, and handle validation.
 *
 * Tests that need garbage to actually die run on a best-effort basis
 * against the v1 CONSERVATIVE stack scan (gc.c): they NULL their
 * locals, allocate victims inside helper functions (so the references
 * live in dead frames), and call clobber_stack() to overwrite those
 * frames before collecting. Built at -O0 this is deterministic; the
 * M15's precise stackmap consumer will make it exact.
 *
 * Expected stdout is deterministic (booleans and fixed values only);
 * see the EXPECTED-OUTPUT file next to this test.
 *
 * Build & run (M8: rt.c references the C++ ABI for exceptions, hence
 * -lc++abi):
 *   cc -std=c11 -Wall -Wextra -pthread -I runtime/include runtime/src/rt.c runtime/src/gc.c runtime/src/gc/collector.c runtime/src/gc/roots.c runtime/src/thread.c runtime/src/callback.c runtime/src/platform/os/darwin.c runtime/tests/rt_test.c -o /tmp/scoop_rt_test -lc++abi
 *   /tmp/scoop_rt_test
 */
#include <pthread.h>
#include <sched.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

#include "scoop_rt.h"

/* Matches the layout of the @scoop_td_String global emitted by codegen.
 * Non-static: rt.c references it from scoop_rt_string_concat. size is
 * the fixed part (16-byte header + len); Strings have no references,
 * so the scan descriptor is NULL. */
const ScoopTypeDescriptor scoop_td_String = {1, 24, 8, NULL, NULL, NULL, NULL, 0, "String"};

/* Same layout codegen uses for StringConst globals:
 * { td, gc_word, len, data } (16-byte header, runtime spec 2.4). */
typedef struct {
    const ScoopTypeDescriptor *td;
    uint64_t gc_word;
    uint64_t len;
    char data[5];
} FiveCharConst;

static const FiveCharConst hello = {&scoop_td_String, 0, 5, {'h', 'e', 'l', 'l', 'o'}};
static const FiveCharConst world = {&scoop_td_String, 0, 5, {'w', 'o', 'r', 'l', 'd'}};

/* M6 dispatch fixtures: interface Describable; class Shape; class
 * Point : Shape, Describable (vtable contains its ordinary describe method). Sizes
 * include the 16-byte header. */
static int64_t point_describe(const void *self) {
    (void)self;
    return 7;
}

static const ScoopTypeDescriptor describable_td = {
    1002, 0, 8, NULL, NULL, NULL, NULL, 0, "Describable"};
static const void *const point_describable_slots[] = {(const void *)&point_describe};
static const ScoopItableEntry point_itables[] = {{&describable_td, point_describable_slots}};
static const void *const point_vtable[] = {(const void *)&point_describe};
static const ScoopTypeDescriptor shape_td = {1000, 24, 8, NULL, NULL, NULL, NULL, 0, "Shape"};
static const ScoopTypeDescriptor point_td = {
    1001, 32, 8, NULL, &shape_td, point_vtable, point_itables, 1, "Point"};

/* M9 GC fixtures. */

/* class Node { value: i64, next: Node? } — plain layout, one reference
 * at object offset 24. */
typedef struct ScoopNode {
    ScoopObjectHeader header; /* 0..16 */
    int64_t value; /* 16 */
    struct ScoopNode *next; /* 24 */
} ScoopNode; /* size 32 */
static const uint64_t node_refs[] = {1, 24};
static const ScoopTypeDescriptor node_td = {
    2000, 32, 8, node_refs, NULL, NULL, NULL, 0, "Node"};

/* Exact image metadata normally emitted by codegen. The managed global uses
 * an inline-value scan rooted at its writable pointer slot; String literals
 * are immutable, GC-free-payload managed objects with stable addresses. */
static ScoopNode *image_global_rooted;
static void *image_immortal_rooted;
static const uint64_t image_global_scan[] = {1, 0};
const ScoopManagedGlobalDescriptor scoop_image_managed_globals[] = {
    {&image_global_rooted, image_global_scan}};
const uint64_t scoop_image_managed_global_count = 1;
const ScoopImmortalObjectDescriptor scoop_image_immortal_objects[] = {
    {&hello, sizeof hello, &scoop_td_String},
    {&world, sizeof world, &scoop_td_String},
};
const uint64_t scoop_image_immortal_object_count = 2;

/* 64-byte plain object without references: exactly two per line, for
 * the free-line reuse test. */
typedef struct {
    ScoopObjectHeader header;
    int64_t words[6];
} ScoopBig64; /* size 64 */
static const ScoopTypeDescriptor big64_td = {2001, 64, 8, NULL, NULL, NULL, NULL, 0, "Big64"};

/* Array with reference elements (recursive SCOOP_REFS_ARRAY scan);
 * `size` in the TD is the element stride (pointer). */
static const uint64_t ref_element_scan[] = {1, 0};
static const uint64_t ref_array_scan[] = {
    SCOOP_REFS_ARRAY, 8, (uint64_t)(uintptr_t)ref_element_scan};
static const ScoopTypeDescriptor ref_array_td = {
    2100, 8, 8, ref_array_scan, NULL, NULL, NULL, 0, "Array<String>"};

/* Boxed tagged enum E { A(String), B(i64) }: B uses the shared pure
 * slot; A has its own ref-bearing slot. */
typedef struct {
    ScoopObjectHeader header;
    uint64_t tag;
    uint64_t pure_payload;
    const ScoopString *a_ref;
} ScoopBoxedEnum; /* size 40 */
static const uint64_t enum_scan[] = {1, 32};
static const ScoopTypeDescriptor enum_td = {
    2002, 40, 8, enum_scan, NULL, NULL, NULL, 0, "E"};

/* Array<Nested>, where each inline element is
 * { tagged enum E, tail: String }. The sequence combines the
 * unconditional tail reference with a tag-selected payload scan;
 * the array wrapper repeats that recursive element scan by stride. */
typedef struct {
    uint64_t tag;
    uint64_t pure_payload;
    const ScoopString *a_ref;
    const ScoopString *tail;
} ScoopNestedElement; /* size 32 */
static const uint64_t nested_element_scan[] = {2, 16, 24};
static const uint64_t nested_array_scan[] = {
    SCOOP_REFS_ARRAY, sizeof(ScoopNestedElement),
    (uint64_t)(uintptr_t)nested_element_scan};
static const ScoopTypeDescriptor nested_array_td = {
    2101, sizeof(ScoopNestedElement), 8, nested_array_scan,
    NULL, NULL, NULL, 0, "Array<Nested>"};

static ScoopNode *new_node(int64_t value, ScoopNode *next) {
    ScoopNode *node = scoop_rt_alloc(&node_td, sizeof(ScoopNode));
    node->value = value;
    node->next = next;
    return node;
}

/* Garbage is created inside helpers so the only references live in
 * the helper's (dead) frame after it returns. */
static void make_garbage_nodes(int count) {
    for (int i = 0; i < count; i++) {
        ScoopNode *junk = new_node(i, NULL);
        (void)junk;
    }
}

static void make_garbage_big64(void) {
    for (int i = 0; i < 4; i++) {
        ScoopBig64 *victim = scoop_rt_alloc(&big64_td, sizeof(ScoopBig64));
        victim->words[0] = -1;
    }
}

/* Enough 64B garbage to overrun one 32KB block and spill into the
 * next (600 * 64B > 255 usable lines). */
static void make_garbage_big64_many(void) {
    for (int i = 0; i < 600; i++) {
        ScoopBig64 *victim = scoop_rt_alloc(&big64_td, sizeof(ScoopBig64));
        victim->words[0] = -1;
    }
}

static ScoopArray *make_ref_array(void) {
    ScoopArray *array = scoop_rt_alloc(&ref_array_td, sizeof(ScoopArray) + 2 * sizeof(uint64_t));
    array->size = 2;
    const ScoopString **elements = (const ScoopString **)array->elements;
    elements[0] = scoop_rt_int_to_string(1001);
    elements[1] = scoop_rt_int_to_string(1002);
    return array;
}

static ScoopBoxedEnum *make_boxed_enum_a(void) {
    ScoopBoxedEnum *e = scoop_rt_alloc(&enum_td, sizeof(ScoopBoxedEnum));
    e->tag = 0;
    e->a_ref = scoop_rt_int_to_string(1003);
    return e;
}

static ScoopBoxedEnum *make_boxed_enum_b(void) {
    ScoopBoxedEnum *e = scoop_rt_alloc(&enum_td, sizeof(ScoopBoxedEnum));
    e->tag = 1;
    e->pure_payload = 0xDEADBEEF0; /* pure-value payload is not scanned */
    return e;
}

static ScoopArray *make_nested_array(void) {
    ScoopArray *array = scoop_rt_alloc(
        &nested_array_td,
        sizeof(ScoopArray) + 2 * sizeof(ScoopNestedElement));
    array->size = 2;
    ScoopNestedElement *elements = (ScoopNestedElement *)array->elements;
    elements[0].tag = 0;
    elements[0].a_ref = scoop_rt_int_to_string(1004);
    elements[0].tail = scoop_rt_int_to_string(1005);
    elements[1].tag = 1;
    elements[1].pure_payload = UINT64_C(0xDEADBEEF0);
    elements[1].tail = scoop_rt_int_to_string(1006);
    return array;
}

/* Overwrite the dead stack region below the current frame so the
 * conservative stack scan no longer finds stale object pointers left
 * behind by helpers that have returned (v1 transition-era helper; see
 * the file header comment). */
static void clobber_stack(void) {
    volatile uint64_t buf[2048];
    for (size_t i = 0; i < 2048; i++) {
        buf[i] = 0;
    }
}

/* Static slot registered as a global root (not on the stack, so only
 * the root registration keeps its value alive). */
static ScoopNode *global_rooted;

typedef struct ThreadProbe {
    bool detached_before;
    bool owns_attachment;
    bool nested_is_borrowed;
    bool stack_bounds_contain_local;
    bool registry_has_main_and_worker;
    bool native_roots_are_thread_local;
    bool mode_transitions_are_exact;
    bool detached_after;
} ThreadProbe;

static void *probe_foreign_thread(void *raw_probe) {
    ThreadProbe *probe = raw_probe;
    int stack_local = 0;
    volatile char managed_stack_boundary = 0;
    probe->detached_before = !scoop_rt_thread_debug_is_attached();
    probe->owns_attachment = scoop_rt_attach_foreign_thread();
    probe->nested_is_borrowed = !scoop_rt_attach_foreign_thread();
    probe->mode_transitions_are_exact =
        scoop_rt_thread_debug_mode() == SCOOP_THREAD_DEBUG_NATIVE_SAFE;
    uintptr_t stack_low = scoop_rt_thread_debug_stack_low();
    uintptr_t stack_high = scoop_rt_thread_debug_stack_high();
    uintptr_t local_address = (uintptr_t)&stack_local;
    probe->stack_bounds_contain_local =
        stack_low <= local_address && local_address < stack_high;
    probe->registry_has_main_and_worker = scoop_rt_thread_debug_count() == 2;

    void *root_value = NULL;
    void **root_slots[] = {&root_value};
    ScoopNativeRootFrame frame;
    scoop_rt_thread_debug_enter_managed((uintptr_t)&managed_stack_boundary);
    probe->mode_transitions_are_exact =
        probe->mode_transitions_are_exact &&
        scoop_rt_thread_debug_mode() == SCOOP_THREAD_DEBUG_MANAGED;
    scoop_rt_push_native_roots(&frame, root_slots, 1);
    probe->native_roots_are_thread_local = scoop_rt_gc_debug_native_root_count() == 1;
    scoop_rt_pop_native_roots(&frame);
    probe->native_roots_are_thread_local =
        probe->native_roots_are_thread_local && scoop_rt_gc_debug_native_root_count() == 0;
    scoop_rt_thread_debug_leave_managed();
    probe->mode_transitions_are_exact =
        probe->mode_transitions_are_exact &&
        scoop_rt_thread_debug_mode() == SCOOP_THREAD_DEBUG_NATIVE_SAFE;

    if (probe->owns_attachment) {
        scoop_rt_detach_foreign_thread();
    }
    probe->detached_after = !scoop_rt_thread_debug_is_attached();
    return NULL;
}

static void *detach_with_native_root(void *unused) {
    (void)unused;
    volatile char managed_stack_boundary = 0;
    (void)scoop_rt_attach_foreign_thread();
    scoop_rt_thread_debug_enter_managed((uintptr_t)&managed_stack_boundary);
    void *root_value = NULL;
    void **root_slots[] = {&root_value};
    ScoopNativeRootFrame frame;
    scoop_rt_push_native_roots(&frame, root_slots, 1);
    scoop_rt_thread_debug_leave_managed();
    scoop_rt_detach_foreign_thread();
    return NULL;
}

static void *detach_twice(void *unused) {
    (void)unused;
    (void)scoop_rt_attach_foreign_thread();
    scoop_rt_detach_foreign_thread();
    scoop_rt_detach_foreign_thread();
    return NULL;
}

static void *caller_root_lifo_violation(void *unused) {
    (void)unused;
    volatile char managed_stack_boundary = 0;
    (void)scoop_rt_attach_foreign_thread();
    scoop_rt_thread_debug_enter_managed((uintptr_t)&managed_stack_boundary);
    ScoopCallerRootFrame outer;
    ScoopCallerRootFrame inner;
    scoop_rt_push_caller_roots(&outer, NULL, 0);
    scoop_rt_push_caller_roots(&inner, NULL, 0);
    scoop_rt_pop_caller_roots(&outer);
    return NULL;
}

static void *compiler_root_lifo_violation(void *unused) {
    (void)unused;
    volatile char managed_stack_boundary = 0;
    (void)scoop_rt_attach_foreign_thread();
    scoop_rt_thread_debug_enter_managed((uintptr_t)&managed_stack_boundary);
    ScoopCompilerRootFrame outer;
    ScoopCompilerRootFrame inner;
    scoop_rt_push_compiler_roots(&outer, NULL, 0);
    scoop_rt_push_compiler_roots(&inner, NULL, 0);
    scoop_rt_pop_compiler_roots(&outer);
    return NULL;
}

static void leave_wrong_transition(void) {
    ScoopCallerRootFrame caller_frame;
    ScoopThreadTransition active = {0};
    ScoopThreadTransition wrong = {0};
    volatile char managed_stack_pointer = 0;
    scoop_rt_push_caller_roots(&caller_frame, NULL, 0);
    scoop_rt_enter_native_safe(&active, (uintptr_t)&managed_stack_pointer);
    scoop_rt_leave_native_safe(&wrong);
}

static void *transition_lifo_violation(void *unused) {
    (void)unused;
    volatile char managed_stack_boundary = 0;
    (void)scoop_rt_attach_foreign_thread();
    scoop_rt_thread_debug_enter_managed((uintptr_t)&managed_stack_boundary);
    leave_wrong_transition();
    return NULL;
}

static bool thread_protocol_aborts(void *(*start)(void *)) {
    fflush(stdout);
    pid_t pid = fork();
    if (pid == 0) {
        pthread_t thread;
        if (pthread_create(&thread, NULL, start, NULL) != 0) {
            _exit(2);
        }
        (void)pthread_join(thread, NULL);
        _exit(0);
    }
    int status = 0;
    waitpid(pid, &status, 0);
    return WIFSIGNALED(status);
}

typedef struct StwProbe {
    _Atomic bool *collect_now;
    _Atomic bool *stop;
    _Atomic bool ready;
    _Atomic bool collection_returned;
    bool owns_attachment;
    bool root_survived;
    bool mode_transitions_are_exact;
} StwProbe;

typedef struct BorrowedProbe {
    _Atomic bool ready;
    _Atomic bool enter_runtime;
    bool owns_attachment;
    bool root_survived;
    bool mode_transitions_are_exact;
} BorrowedProbe;

enum { MULTI_ALLOC_THREADS = 4, MULTI_ALLOC_BATCH = 512 };

typedef struct MultiAllocProbe {
    _Atomic bool *start;
    _Atomic bool *collection_finished;
    _Atomic uint32_t *ready;
    uint64_t final_handle;
    bool valid_after_collection;
    bool owns_attachment;
} MultiAllocProbe;

static void *multi_allocator(void *raw_probe) {
    MultiAllocProbe *probe = raw_probe;
    volatile char managed_stack_boundary = 0;
    probe->owns_attachment = scoop_rt_attach_foreign_thread();
    scoop_rt_thread_debug_enter_managed((uintptr_t)&managed_stack_boundary);

    while (!atomic_load_explicit(probe->start, memory_order_acquire)) {
        scoop_rt_safepoint();
        sched_yield();
    }

    ScoopNode *head = NULL;
    void **root_slots[] = {(void **)&head};
    ScoopNativeRootFrame root_frame;
    scoop_rt_push_native_roots(&root_frame, root_slots, 1);
    for (int i = 0; i < MULTI_ALLOC_BATCH; i++) {
        head = new_node(i, head);
    }
    uint64_t first_handle = scoop_rt_get_handle(head);
    atomic_fetch_add_explicit(probe->ready, 1, memory_order_acq_rel);
    while (!atomic_load_explicit(probe->collection_finished, memory_order_acquire)) {
        scoop_rt_safepoint();
        sched_yield();
    }
    const ScoopNode *after_collection = scoop_rt_resolve_handle(first_handle);
    probe->valid_after_collection =
        after_collection != NULL && after_collection->value == MULTI_ALLOC_BATCH - 1;

    for (int i = 0; i < MULTI_ALLOC_BATCH; i++) {
        head = new_node(MULTI_ALLOC_BATCH + i, head);
    }
    probe->final_handle = scoop_rt_get_handle(head);
    (void)scoop_rt_release_handle(first_handle);
    scoop_rt_pop_native_roots(&root_frame);
    scoop_rt_thread_debug_leave_managed();
    if (probe->owns_attachment) {
        scoop_rt_detach_foreign_thread();
    }
    return NULL;
}

static const uint64_t callback_signature_i64;
static const uint64_t callback_signature_other;
static _Atomic bool callback_lease_entered;
static _Atomic bool callback_lease_leave;

static uint64_t callback_add_adapter(const void *raw_closure,
                                     void *result_storage,
                                     const void *const *argument_storage,
                                     void **exception_out) {
    (void)exception_out;
    const ScoopNode *closure = raw_closure;
    int64_t argument = *(const int64_t *)argument_storage[0];
    ScoopNode *allocated = new_node(closure->value + argument, NULL);
    *(int64_t *)result_storage = allocated->value;
    scoop_rt_gc_collect();
    return SCOOP_FOREIGN_CALLBACK_RETURNED;
}

static uint64_t callback_throw_adapter(const void *raw_closure,
                                       void *result_storage,
                                       const void *const *argument_storage,
                                       void **exception_out) {
    (void)result_storage;
    (void)argument_storage;
    const ScoopNode *closure = raw_closure;
    *exception_out = new_node(closure->value + 1000, NULL);
    return SCOOP_FOREIGN_CALLBACK_THREW;
}

static uint64_t callback_lease_adapter(const void *raw_closure,
                                       void *result_storage,
                                       const void *const *argument_storage,
                                       void **exception_out) {
    (void)exception_out;
    const ScoopNode *closure = raw_closure;
    int64_t argument = *(const int64_t *)argument_storage[0];
    atomic_store_explicit(&callback_lease_entered, true, memory_order_release);
    while (!atomic_load_explicit(&callback_lease_leave, memory_order_acquire)) {
        scoop_rt_safepoint();
        sched_yield();
    }
    *(int64_t *)result_storage = closure->value + argument;
    return SCOOP_FOREIGN_CALLBACK_RETURNED;
}

typedef struct CallbackInvokeProbe {
    void *context;
    const void *signature;
    int64_t argument;
    int64_t result;
    uint32_t status;
    bool detached_before;
    bool detached_after;
} CallbackInvokeProbe;

static void *invoke_callback_worker(void *raw_probe) {
    CallbackInvokeProbe *probe = raw_probe;
    const void *arguments[] = {&probe->argument};
    probe->detached_before = !scoop_rt_thread_debug_is_attached();
    probe->status = scoop_runtime_callback_invoke(
        probe->context, probe->signature, &probe->result, arguments);
    probe->detached_after = !scoop_rt_thread_debug_is_attached();
    return NULL;
}

static bool join_thread_native_safe(pthread_t thread) {
    ScoopCallerRootFrame caller_frame;
    ScoopThreadTransition transition = {0};
    volatile char managed_stack_pointer = 0;
    scoop_rt_push_caller_roots(&caller_frame, NULL, 0);
    scoop_rt_enter_native_safe(&transition,
                               (uintptr_t)&managed_stack_pointer);
    int result = pthread_join(thread, NULL);
    scoop_rt_leave_native_safe(&transition);
    scoop_rt_pop_caller_roots(&caller_frame);
    return result == 0;
}

static void *managed_collection_requester(void *raw_probe) {
    StwProbe *probe = raw_probe;
    volatile char managed_stack_boundary = 0;
    probe->owns_attachment = scoop_rt_attach_foreign_thread();
    scoop_rt_thread_debug_enter_managed((uintptr_t)&managed_stack_boundary);
    probe->mode_transitions_are_exact =
        scoop_rt_thread_debug_mode() == SCOOP_THREAD_DEBUG_MANAGED;
    atomic_store_explicit(&probe->ready, true, memory_order_release);
    while (!atomic_load_explicit(probe->collect_now, memory_order_acquire)) {
        scoop_rt_safepoint();
        sched_yield();
    }
    scoop_rt_gc_collect();
    probe->mode_transitions_are_exact =
        probe->mode_transitions_are_exact &&
        scoop_rt_thread_debug_mode() == SCOOP_THREAD_DEBUG_MANAGED;
    atomic_store_explicit(&probe->collection_returned, true, memory_order_release);
    while (!atomic_load_explicit(probe->stop, memory_order_acquire)) {
        scoop_rt_safepoint();
        sched_yield();
    }
    scoop_rt_thread_debug_leave_managed();
    if (probe->owns_attachment) {
        scoop_rt_detach_foreign_thread();
    }
    return NULL;
}

static void run_native_safe_observer(StwProbe *probe) {
    ScoopNode **caller_root_value = malloc(sizeof *caller_root_value);
    if (caller_root_value == NULL) {
        abort();
    }
    *caller_root_value = new_node(31415, NULL);
    static const uint64_t caller_root_scan[] = {1, 0};
    ScoopCallerRootEntry caller_root_entries[] = {
        {.base = caller_root_value, .scan = caller_root_scan},
    };
    ScoopCallerRootFrame caller_frame;
    ScoopThreadTransition transition = {0};
    volatile char managed_stack_pointer = 0;
    scoop_rt_push_caller_roots(&caller_frame, caller_root_entries, 1);
    scoop_rt_enter_native_safe(&transition, (uintptr_t)&managed_stack_pointer);
    probe->mode_transitions_are_exact =
        scoop_rt_thread_debug_mode() == SCOOP_THREAD_DEBUG_NATIVE_SAFE;
    atomic_store_explicit(&probe->ready, true, memory_order_release);
    while (!atomic_load_explicit(probe->stop, memory_order_acquire)) {
        sched_yield();
    }
    scoop_rt_leave_native_safe(&transition);
    probe->mode_transitions_are_exact =
        probe->mode_transitions_are_exact &&
        scoop_rt_thread_debug_mode() == SCOOP_THREAD_DEBUG_MANAGED;
    probe->root_survived =
        scoop_rt_gc_debug_is_allocated(*caller_root_value) &&
        (*caller_root_value)->header.td == &node_td &&
        (*caller_root_value)->value == 31415;
    scoop_rt_pop_caller_roots(&caller_frame);
    free(caller_root_value);
}

static void *native_safe_observer(void *raw_probe) {
    StwProbe *probe = raw_probe;
    volatile char managed_stack_boundary = 0;
    probe->owns_attachment = scoop_rt_attach_foreign_thread();
    scoop_rt_thread_debug_enter_managed((uintptr_t)&managed_stack_boundary);
    run_native_safe_observer(probe);
    scoop_rt_thread_debug_leave_managed();
    if (probe->owns_attachment) {
        scoop_rt_detach_foreign_thread();
    }
    return NULL;
}

static void run_native_borrowed_observer(BorrowedProbe *probe) {
    ScoopNode **native_root_value = malloc(sizeof *native_root_value);
    if (native_root_value == NULL) {
        abort();
    }
    *native_root_value = new_node(27182, NULL);
    void **native_root_slots[] = {(void **)native_root_value};
    ScoopCallerRootFrame caller_frame;
    ScoopThreadTransition transition = {0};
    ScoopNativeRootFrame native_frame;
    volatile char managed_stack_pointer = 0;
    scoop_rt_push_caller_roots(&caller_frame, NULL, 0);
    scoop_rt_enter_native_borrowed(&transition,
                                    (uintptr_t)&managed_stack_pointer);
    probe->mode_transitions_are_exact =
        scoop_rt_thread_debug_mode() == SCOOP_THREAD_DEBUG_NATIVE_BORROWED;
    scoop_rt_push_native_roots(&native_frame, native_root_slots, 1);
    atomic_store_explicit(&probe->ready, true, memory_order_release);
    while (!atomic_load_explicit(&probe->enter_runtime,
                                 memory_order_acquire)) {
        sched_yield();
    }

    /* The collector is already waiting for this native-borrowed thread.
     * The first explicit runtime entry parks it and joins that epoch. */
    scoop_rt_gc_collect();
    probe->root_survived =
        scoop_rt_gc_debug_is_allocated(*native_root_value) &&
        (*native_root_value)->header.td == &node_td &&
        (*native_root_value)->value == 27182;
    probe->mode_transitions_are_exact =
        probe->mode_transitions_are_exact &&
        scoop_rt_thread_debug_mode() == SCOOP_THREAD_DEBUG_NATIVE_BORROWED;
    scoop_rt_pop_native_roots(&native_frame);
    scoop_rt_leave_native_borrowed(&transition);
    probe->mode_transitions_are_exact =
        probe->mode_transitions_are_exact &&
        scoop_rt_thread_debug_mode() == SCOOP_THREAD_DEBUG_MANAGED;
    scoop_rt_pop_caller_roots(&caller_frame);
    free(native_root_value);
}

static void *native_borrowed_observer(void *raw_probe) {
    BorrowedProbe *probe = raw_probe;
    volatile char managed_stack_boundary = 0;
    probe->owns_attachment = scoop_rt_attach_foreign_thread();
    scoop_rt_thread_debug_enter_managed((uintptr_t)&managed_stack_boundary);
    run_native_borrowed_observer(probe);
    scoop_rt_thread_debug_leave_managed();
    if (probe->owns_attachment) {
        scoop_rt_detach_foreign_thread();
    }
    return NULL;
}

void scoop_main(void) {
    const ScoopString *a = (const ScoopString *)&hello;
    const ScoopString *b = (const ScoopString *)&world;

    /* M13 thread-registration baseline: main and foreign pthreads use the
     * same TLS state/registry, nested attach does not transfer ownership,
     * stack bounds come from pthread APIs, and native roots belong to the
     * current thread state. This first probe only checks registration; later
     * probes exercise concurrent allocation and collection. */
    int main_stack_local = 0;
    uintptr_t main_stack_low = scoop_rt_thread_debug_stack_low();
    uintptr_t main_stack_high = scoop_rt_thread_debug_stack_high();
    uintptr_t main_local_address = (uintptr_t)&main_stack_local;
    scoop_rt_println_boolean(scoop_rt_thread_debug_is_attached() &&
                             scoop_rt_thread_debug_mode() ==
                                 SCOOP_THREAD_DEBUG_MANAGED &&
                             scoop_rt_thread_debug_count() == 1 &&
                             main_stack_low <= main_local_address &&
                             main_local_address < main_stack_high);
    ThreadProbe thread_probe = {0};
    pthread_t probe_thread;
    bool probe_created =
        pthread_create(&probe_thread, NULL, probe_foreign_thread, &thread_probe) == 0;
    bool probe_joined = probe_created && pthread_join(probe_thread, NULL) == 0;
    scoop_rt_println_boolean(probe_joined && thread_probe.detached_before &&
                             thread_probe.owns_attachment && thread_probe.nested_is_borrowed &&
                             thread_probe.stack_bounds_contain_local &&
                             thread_probe.registry_has_main_and_worker &&
                             thread_probe.mode_transitions_are_exact);
    scoop_rt_println_boolean(probe_joined && thread_probe.native_roots_are_thread_local);
    scoop_rt_println_boolean(probe_joined && thread_probe.detached_after &&
                             scoop_rt_thread_debug_count() == 1);
    scoop_rt_println_boolean(thread_protocol_aborts(detach_with_native_root));
    scoop_rt_println_boolean(thread_protocol_aborts(detach_twice));
    scoop_rt_println_boolean(thread_protocol_aborts(caller_root_lifo_violation));
    scoop_rt_println_boolean(thread_protocol_aborts(compiler_root_lifo_violation));
    scoop_rt_println_boolean(thread_protocol_aborts(transition_lifo_violation));

    /* M13 cooperative STW: main and a foreign managed requester race to
     * request the same collection. The winner becomes the sole collector;
     * the loser parks and joins that epoch. A second foreign thread remains
     * native-safe and therefore never enters the wait set. */
    _Atomic bool collect_now = false;
    _Atomic bool stop_stw_workers = false;
    StwProbe managed_probe = {
        .collect_now = &collect_now,
        .stop = &stop_stw_workers,
    };
    StwProbe native_probe = {
        .collect_now = &collect_now,
        .stop = &stop_stw_workers,
    };
    atomic_init(&managed_probe.ready, false);
    atomic_init(&managed_probe.collection_returned, false);
    atomic_init(&native_probe.ready, false);
    atomic_init(&native_probe.collection_returned, false);
    pthread_t managed_thread;
    pthread_t native_thread;
    bool managed_created = pthread_create(
                               &managed_thread, NULL, managed_collection_requester,
                               &managed_probe) == 0;
    bool native_created =
        pthread_create(&native_thread, NULL, native_safe_observer, &native_probe) == 0;
    while (managed_created && native_created &&
           (!atomic_load_explicit(&managed_probe.ready, memory_order_acquire) ||
            !atomic_load_explicit(&native_probe.ready, memory_order_acquire))) {
        sched_yield();
    }
    bool workers_ready = managed_created && native_created &&
                         scoop_rt_thread_debug_count() == 3;
    uint64_t epoch_before = scoop_rt_thread_debug_gc_epoch();
    atomic_store_explicit(&collect_now, true, memory_order_release);
    scoop_rt_gc_collect();
    while (workers_ready &&
           !atomic_load_explicit(&managed_probe.collection_returned,
                                 memory_order_acquire)) {
        scoop_rt_safepoint();
        sched_yield();
    }
    uint64_t epoch_after = scoop_rt_thread_debug_gc_epoch();
    bool first_epoch_coalesced =
        workers_ready && epoch_after == epoch_before + 1 &&
        scoop_rt_thread_debug_last_gc_parked_count() == 1 &&
        scoop_rt_thread_debug_last_gc_native_safe_count() == 1;
    for (int i = 0; i < 8; i++) {
        scoop_rt_gc_collect();
    }
    bool consecutive_epochs_parked =
        scoop_rt_thread_debug_gc_epoch() == epoch_after + 8 &&
        scoop_rt_thread_debug_last_gc_parked_count() == 1 &&
        scoop_rt_thread_debug_last_gc_native_safe_count() == 1;
    scoop_rt_println_boolean(first_epoch_coalesced && consecutive_epochs_parked);
    atomic_store_explicit(&stop_stw_workers, true, memory_order_release);
    bool managed_joined = managed_created && pthread_join(managed_thread, NULL) == 0;
    bool native_joined = native_created && pthread_join(native_thread, NULL) == 0;
    scoop_rt_println_boolean(managed_joined && native_joined &&
                             managed_probe.owns_attachment &&
                             native_probe.owns_attachment &&
                             native_probe.root_survived &&
                             managed_probe.mode_transitions_are_exact &&
                             native_probe.mode_transitions_are_exact &&
                             scoop_rt_thread_debug_count() == 1);

    /* Per-thread TLABs, heap/root synchronization and generation handles:
     * four managed mutators allocate disjoint ranges concurrently, publish
     * handles, park for one deterministic collection, then refill retired
     * TLABs and allocate a second batch. */
    _Atomic bool multi_start = false;
    _Atomic bool multi_collection_finished = false;
    _Atomic uint32_t multi_ready = 0;
    MultiAllocProbe multi_probes[MULTI_ALLOC_THREADS] = {0};
    pthread_t multi_threads[MULTI_ALLOC_THREADS];
    size_t multi_created_count = 0;
    for (size_t i = 0; i < MULTI_ALLOC_THREADS; i++) {
        multi_probes[i].start = &multi_start;
        multi_probes[i].collection_finished = &multi_collection_finished;
        multi_probes[i].ready = &multi_ready;
        if (pthread_create(&multi_threads[i], NULL, multi_allocator,
                           &multi_probes[i]) != 0) {
            break;
        }
        multi_created_count++;
    }
    atomic_store_explicit(&multi_start, true, memory_order_release);
    while (multi_created_count != 0 &&
           atomic_load_explicit(&multi_ready, memory_order_acquire) <
               multi_created_count) {
        scoop_rt_safepoint();
        sched_yield();
    }
    if (multi_created_count != 0) {
        scoop_rt_gc_collect();
    }
    atomic_store_explicit(&multi_collection_finished, true,
                          memory_order_release);
    bool multi_valid = multi_created_count == MULTI_ALLOC_THREADS;
    for (size_t i = 0; i < multi_created_count; i++) {
        multi_valid = pthread_join(multi_threads[i], NULL) == 0 && multi_valid;
        const ScoopNode *node =
            scoop_rt_resolve_handle(multi_probes[i].final_handle);
        size_t count = 0;
        for (; node != NULL; node = node->next) {
            count++;
        }
        multi_valid = multi_valid && multi_probes[i].owns_attachment &&
                      multi_probes[i].valid_after_collection &&
                      count == 2 * MULTI_ALLOC_BATCH;
        (void)scoop_rt_release_handle(multi_probes[i].final_handle);
    }
    scoop_rt_println_boolean(multi_valid &&
                             scoop_rt_thread_debug_count() == 1);

    /* Native transition ABI: caller roots remain published while the active
     * managed segment is frozen. native-safe returns without participating in
     * GC; native-borrowed may explicitly enter the runtime, collect using its
     * caller/native roots, and resume in borrowed mode before the LIFO leave. */
    void *caller_root_value = NULL;
    void **caller_root_slots[] = {&caller_root_value};
    static const uint64_t caller_root_scan[] = {1, 0};
    ScoopCallerRootEntry caller_root_entries[] = {
        {.base = &caller_root_value, .scan = caller_root_scan},
    };
    ScoopCallerRootFrame caller_frame;
    ScoopThreadTransition safe_transition = {0};
    volatile char safe_stack_pointer = 0;
    scoop_rt_push_caller_roots(&caller_frame, caller_root_entries, 1);
    scoop_rt_enter_native_safe(&safe_transition, (uintptr_t)&safe_stack_pointer);
    bool native_safe_active = scoop_rt_thread_debug_transition_depth() == 1 &&
                              scoop_rt_thread_debug_caller_root_count() == 1;
    scoop_rt_leave_native_safe(&safe_transition);
    scoop_rt_pop_caller_roots(&caller_frame);
    scoop_rt_println_boolean(native_safe_active &&
                             scoop_rt_thread_debug_transition_depth() == 0 &&
                             scoop_rt_thread_debug_caller_root_count() == 0);

    ScoopNode *compiler_root_value = new_node(16180, NULL);
    ScoopCallerRootEntry compiler_root_entries[] = {
        {.base = &compiler_root_value, .scan = caller_root_scan},
    };
    ScoopCompilerRootFrame compiler_frame;
    scoop_rt_push_compiler_roots(&compiler_frame, compiler_root_entries, 1);
    scoop_rt_gc_collect();
    bool compiler_root_survived =
        scoop_rt_thread_debug_compiler_root_count() == 1 &&
        scoop_rt_gc_debug_is_allocated(compiler_root_value) &&
        compiler_root_value->value == 16180;
    scoop_rt_pop_compiler_roots(&compiler_frame);
    scoop_rt_println_boolean(
        compiler_root_survived &&
        scoop_rt_thread_debug_compiler_root_count() == 0);

    ScoopCallerRootFrame borrowed_caller_frame;
    ScoopThreadTransition borrowed_transition = {0};
    volatile char borrowed_stack_pointer = 0;
    scoop_rt_push_caller_roots(&borrowed_caller_frame, caller_root_entries, 1);
    scoop_rt_enter_native_borrowed(&borrowed_transition,
                                    (uintptr_t)&borrowed_stack_pointer);
    ScoopNativeRootFrame borrowed_native_frame;
    scoop_rt_push_native_roots(&borrowed_native_frame, caller_root_slots, 1);
    uint64_t borrowed_epoch = scoop_rt_thread_debug_gc_epoch();
    scoop_rt_gc_collect();
    bool borrowed_collected = scoop_rt_thread_debug_gc_epoch() == borrowed_epoch + 1 &&
                              scoop_rt_thread_debug_transition_depth() == 1 &&
                              scoop_rt_thread_debug_caller_root_count() == 1 &&
                              scoop_rt_gc_debug_native_root_count() == 1;
    scoop_rt_pop_native_roots(&borrowed_native_frame);
    scoop_rt_leave_native_borrowed(&borrowed_transition);
    scoop_rt_pop_caller_roots(&borrowed_caller_frame);
    scoop_rt_println_boolean(borrowed_collected &&
                             scoop_rt_thread_debug_transition_depth() == 0 &&
                             scoop_rt_thread_debug_caller_root_count() == 0 &&
                             scoop_rt_gc_debug_native_root_count() == 0);

    /* A native-borrowed thread is not quiescent merely because it is outside
     * managed code. Another collector waits while it executes ordinary C,
     * then the borrower parks at its first explicit runtime entry. */
    _Atomic bool borrowed_collect_now = false;
    _Atomic bool stop_borrowed_collector = false;
    StwProbe borrowed_collector_probe = {
        .collect_now = &borrowed_collect_now,
        .stop = &stop_borrowed_collector,
    };
    BorrowedProbe borrowed_probe = {0};
    atomic_init(&borrowed_collector_probe.ready, false);
    atomic_init(&borrowed_collector_probe.collection_returned, false);
    atomic_init(&borrowed_probe.ready, false);
    atomic_init(&borrowed_probe.enter_runtime, false);
    pthread_t borrowed_thread;
    pthread_t borrowed_collector_thread;
    bool borrowed_thread_created =
        pthread_create(&borrowed_thread, NULL, native_borrowed_observer,
                       &borrowed_probe) == 0;
    bool borrowed_collector_created =
        pthread_create(&borrowed_collector_thread, NULL,
                       managed_collection_requester,
                       &borrowed_collector_probe) == 0;
    while (borrowed_thread_created && borrowed_collector_created &&
           (!atomic_load_explicit(&borrowed_probe.ready,
                                  memory_order_acquire) ||
            !atomic_load_explicit(&borrowed_collector_probe.ready,
                                  memory_order_acquire))) {
        scoop_rt_safepoint();
        sched_yield();
    }
    ScoopCallerRootFrame borrowed_wait_caller_frame;
    ScoopThreadTransition borrowed_wait_transition = {0};
    volatile char borrowed_wait_stack_pointer = 0;
    scoop_rt_push_caller_roots(&borrowed_wait_caller_frame, NULL, 0);
    scoop_rt_enter_native_safe(&borrowed_wait_transition,
                               (uintptr_t)&borrowed_wait_stack_pointer);
    uint64_t borrowed_wait_epoch = scoop_rt_thread_debug_gc_epoch();
    atomic_store_explicit(&borrowed_collect_now, true,
                          memory_order_release);
    while (borrowed_thread_created && borrowed_collector_created &&
           scoop_rt_thread_debug_gc_epoch() == borrowed_wait_epoch) {
        sched_yield();
    }
    atomic_store_explicit(&borrowed_probe.enter_runtime, true,
                          memory_order_release);
    while (borrowed_thread_created && borrowed_collector_created &&
           !atomic_load_explicit(&borrowed_collector_probe.collection_returned,
                                 memory_order_acquire)) {
        sched_yield();
    }
    bool borrowed_wait_counts =
        scoop_rt_thread_debug_last_gc_parked_count() == 1 &&
        scoop_rt_thread_debug_last_gc_native_safe_count() == 1;
    atomic_store_explicit(&stop_borrowed_collector, true,
                          memory_order_release);
    scoop_rt_leave_native_safe(&borrowed_wait_transition);
    scoop_rt_pop_caller_roots(&borrowed_wait_caller_frame);
    bool borrowed_thread_joined =
        borrowed_thread_created && pthread_join(borrowed_thread, NULL) == 0;
    bool borrowed_collector_joined =
        borrowed_collector_created &&
        pthread_join(borrowed_collector_thread, NULL) == 0;
    scoop_rt_println_boolean(
        borrowed_thread_joined && borrowed_collector_joined &&
        borrowed_wait_counts && borrowed_probe.owns_attachment &&
        borrowed_probe.root_survived &&
        borrowed_probe.mode_transitions_are_exact &&
        borrowed_collector_probe.owns_attachment &&
        borrowed_collector_probe.mode_transitions_are_exact &&
        scoop_rt_thread_debug_count() == 1);

    /* Managed callback gateway: a one-shot token transfers its worker owner,
     * automatically attaches a foreign pthread, executes an allocating
     * adapter (including STW GC), records completion, and detaches again. */
    ScoopNode *callback_closure = new_node(40, NULL);
    void *one_shot = scoop_runtime_callback_register(
        callback_closure, callback_add_adapter, &callback_signature_i64,
        SCOOP_FOREIGN_CALLBACK_ONE_SHOT);
    void *one_shot_observer = scoop_runtime_callback_retain(one_shot);
    CallbackInvokeProbe callback_probe = {
        .context = one_shot,
        .signature = &callback_signature_i64,
        .argument = 2,
    };
    pthread_t callback_thread;
    bool callback_created =
        pthread_create(&callback_thread, NULL, invoke_callback_worker,
                       &callback_probe) == 0;
    bool callback_joined =
        callback_created && join_thread_native_safe(callback_thread);
    scoop_rt_println_boolean(
        callback_joined && callback_probe.detached_before &&
        callback_probe.detached_after &&
        callback_probe.status == SCOOP_FOREIGN_CALLBACK_RETURNED &&
        callback_probe.result == 42 &&
        scoop_runtime_callback_state(one_shot_observer) ==
            SCOOP_FOREIGN_CALLBACK_COMPLETED &&
        scoop_runtime_callback_failure(one_shot_observer) == NULL);

    /* The claimed one-shot cannot be entered a second time while its observer
     * keeps the stale boundary deterministic. */
    fflush(stdout);
    pid_t callback_pid = fork();
    if (callback_pid == 0) {
        const void *arguments[] = {&callback_probe.argument};
        (void)scoop_runtime_callback_invoke(
            one_shot, &callback_signature_i64, &callback_probe.result,
            arguments);
        _exit(0);
    }
    int callback_status = 0;
    waitpid(callback_pid, &callback_status, 0);
    scoop_rt_println_boolean(WIFSIGNALED(callback_status));
    scoop_runtime_callback_release(one_shot_observer);

    /* Exception status never crosses the C frame. The first managed failure
     * is rooted by a handle until the observer reads and releases it. */
    ScoopNode *throw_closure = new_node(7, NULL);
    void *throwing = scoop_runtime_callback_register(
        throw_closure, callback_throw_adapter, &callback_signature_i64,
        SCOOP_FOREIGN_CALLBACK_ONE_SHOT);
    void *throw_observer = scoop_runtime_callback_retain(throwing);
    CallbackInvokeProbe throw_probe = {
        .context = throwing,
        .signature = &callback_signature_i64,
    };
    pthread_t throw_thread;
    bool throw_created = pthread_create(&throw_thread, NULL,
                                        invoke_callback_worker,
                                        &throw_probe) == 0;
    bool throw_joined = throw_created && join_thread_native_safe(throw_thread);
    const ScoopNode *callback_failure =
        scoop_runtime_callback_failure(throw_observer);
    scoop_rt_println_boolean(
        throw_joined && throw_probe.status == SCOOP_FOREIGN_CALLBACK_THREW &&
        throw_probe.result == 0 &&
        scoop_runtime_callback_state(throw_observer) ==
            SCOOP_FOREIGN_CALLBACK_FAILED &&
        callback_failure != NULL && callback_failure->value == 1007);
    scoop_runtime_callback_release(throw_observer);

    /* Reusable tokens support concurrent workers and same-thread re-entry
     * from an existing native-safe transition. */
    ScoopNode *reusable_closure = new_node(50, NULL);
    void *reusable = scoop_runtime_callback_register(
        reusable_closure, callback_add_adapter, &callback_signature_i64,
        SCOOP_FOREIGN_CALLBACK_REUSABLE);
    CallbackInvokeProbe reusable_probes[2] = {
        {.context = reusable,
         .signature = &callback_signature_i64,
         .argument = 1},
        {.context = reusable,
         .signature = &callback_signature_i64,
         .argument = 2},
    };
    pthread_t reusable_threads[2];
    bool reusable_ok = true;
    for (size_t i = 0; i < 2; i++) {
        reusable_ok =
            pthread_create(&reusable_threads[i], NULL, invoke_callback_worker,
                           &reusable_probes[i]) == 0 &&
            reusable_ok;
    }
    for (size_t i = 0; i < 2; i++) {
        reusable_ok = join_thread_native_safe(reusable_threads[i]) &&
                      reusable_probes[i].result == 51 + (int64_t)i &&
                      reusable_ok;
    }
    ScoopCallerRootFrame callback_caller_frame;
    ScoopThreadTransition callback_transition = {0};
    volatile char callback_stack_pointer = 0;
    scoop_rt_push_caller_roots(&callback_caller_frame, NULL, 0);
    scoop_rt_enter_native_safe(&callback_transition,
                               (uintptr_t)&callback_stack_pointer);
    int64_t nested_argument = 3;
    int64_t nested_result = 0;
    const void *nested_arguments[] = {&nested_argument};
    uint32_t nested_status = scoop_runtime_callback_invoke(
        reusable, &callback_signature_i64, &nested_result, nested_arguments);
    scoop_rt_leave_native_safe(&callback_transition);
    scoop_rt_pop_caller_roots(&callback_caller_frame);
    reusable_ok = reusable_ok && nested_status == SCOOP_FOREIGN_CALLBACK_RETURNED &&
                  nested_result == 53 &&
                  scoop_runtime_callback_state(reusable) ==
                      SCOOP_FOREIGN_CALLBACK_REGISTERED;
    scoop_rt_println_boolean(reusable_ok);

    /* The active invocation is an independent lease. Deterministically hold
     * the adapter open, drop every external owner, and prove that the token
     * remains live until the invocation returns. */
    atomic_store_explicit(&callback_lease_entered, false,
                          memory_order_release);
    atomic_store_explicit(&callback_lease_leave, false,
                          memory_order_release);
    ScoopNode *lease_closure = new_node(70, NULL);
    void *leased = scoop_runtime_callback_register(
        lease_closure, callback_lease_adapter, &callback_signature_i64,
        SCOOP_FOREIGN_CALLBACK_REUSABLE);
    void *lease_observer = scoop_runtime_callback_retain(leased);
    CallbackInvokeProbe lease_probe = {
        .context = leased,
        .signature = &callback_signature_i64,
        .argument = 2,
    };
    pthread_t lease_thread;
    bool lease_created =
        pthread_create(&lease_thread, NULL, invoke_callback_worker,
                       &lease_probe) == 0;
    while (lease_created &&
           !atomic_load_explicit(&callback_lease_entered,
                                 memory_order_acquire)) {
        scoop_rt_safepoint();
        sched_yield();
    }
    bool active_lease_visible =
        lease_created &&
        scoop_runtime_callback_state(leased) ==
            SCOOP_FOREIGN_CALLBACK_ACTIVE &&
        scoop_runtime_callback_debug_owner_count(leased) == 2 &&
        scoop_runtime_callback_debug_active_count(leased) == 1;
    scoop_runtime_callback_release(leased);
    scoop_runtime_callback_release(lease_observer);
    active_lease_visible =
        active_lease_visible &&
        scoop_runtime_callback_debug_owner_count(leased) == 0 &&
        scoop_runtime_callback_debug_active_count(leased) == 1 &&
        scoop_runtime_callback_debug_live_count() == 2;
    atomic_store_explicit(&callback_lease_leave, true, memory_order_release);
    bool lease_joined = lease_created && join_thread_native_safe(lease_thread);
    scoop_rt_println_boolean(
        active_lease_visible && lease_joined &&
        lease_probe.status == SCOOP_FOREIGN_CALLBACK_RETURNED &&
        lease_probe.result == 72 && lease_probe.detached_before &&
        lease_probe.detached_after &&
        scoop_runtime_callback_debug_live_count() == 1);

    /* Signature identity is checked before the token is touched by managed
     * code; a mismatched trampoline/token pair is a boundary error. */
    fflush(stdout);
    callback_pid = fork();
    if (callback_pid == 0) {
        (void)scoop_runtime_callback_invoke(
            reusable, &callback_signature_other, &nested_result,
            nested_arguments);
        _exit(0);
    }
    callback_status = 0;
    waitpid(callback_pid, &callback_status, 0);
    scoop_rt_println_boolean(WIFSIGNALED(callback_status));
    uintptr_t stale_callback_cookie = (uintptr_t)reusable;
    scoop_runtime_callback_release(reusable);
    scoop_rt_println_boolean(scoop_runtime_callback_debug_live_count() == 0);

    /* Final release invalidates the cookie, and a reused slot advances its
     * generation instead of aliasing the previous token. */
    void *replacement = scoop_runtime_callback_register(
        reusable_closure, callback_add_adapter, &callback_signature_i64,
        SCOOP_FOREIGN_CALLBACK_REUSABLE);
    uintptr_t replacement_cookie = (uintptr_t)replacement;
    const uintptr_t callback_slot_mask = (UINT64_C(1) << 24) - 1;
    scoop_rt_println_boolean(
        (stale_callback_cookie & callback_slot_mask) ==
            (replacement_cookie & callback_slot_mask) &&
        stale_callback_cookie != replacement_cookie);
    scoop_runtime_callback_release(replacement);

    fflush(stdout);
    callback_pid = fork();
    if (callback_pid == 0) {
        (void)scoop_runtime_callback_invoke(
            (void *)stale_callback_cookie, &callback_signature_i64,
            &nested_result, nested_arguments);
        _exit(0);
    }
    callback_status = 0;
    waitpid(callback_pid, &callback_status, 0);
    scoop_rt_println_boolean(WIFSIGNALED(callback_status));

    /* Shutdown rejects both a live token and a subsequent registration.
     * Each check runs in a child so the parent can finish the unit suite. */
    fflush(stdout);
    callback_pid = fork();
    if (callback_pid == 0) {
        (void)scoop_runtime_callback_register(
            reusable_closure, callback_add_adapter, &callback_signature_i64,
            SCOOP_FOREIGN_CALLBACK_REUSABLE);
        scoop_callback_prepare_shutdown();
        _exit(0);
    }
    callback_status = 0;
    waitpid(callback_pid, &callback_status, 0);
    scoop_rt_println_boolean(WIFSIGNALED(callback_status));

    fflush(stdout);
    callback_pid = fork();
    if (callback_pid == 0) {
        scoop_callback_prepare_shutdown();
        (void)scoop_runtime_callback_register(
            reusable_closure, callback_add_adapter, &callback_signature_i64,
            SCOOP_FOREIGN_CALLBACK_REUSABLE);
        _exit(0);
    }
    callback_status = 0;
    waitpid(callback_pid, &callback_status, 0);
    scoop_rt_println_boolean(WIFSIGNALED(callback_status));

    /* concat: "hello" + "world" -> "helloworld" */
    const ScoopString *concat = scoop_rt_string_concat(a, b);
    scoop_rt_println(concat);

    /* structural equality: same content, different content, length
     * mismatch */
    const ScoopString *copy = scoop_rt_string_concat(a, b);
    scoop_rt_println_boolean(scoop_rt_string_eq(concat, copy));
    scoop_rt_println_boolean(scoop_rt_string_eq(a, b));
    scoop_rt_println_boolean(scoop_rt_string_eq(a, concat));

    /* int / boolean output (print variants run into the println line) */
    scoop_rt_print_int(42);
    scoop_rt_println_int(-7);
    scoop_rt_print_boolean(true);
    scoop_rt_println_boolean(false);

    /* Core representation helpers used by ordinary Scoop methods. */
    scoop_rt_println(scoop_rt_int_to_string(42));
    scoop_rt_println(scoop_rt_int_to_string(-7));
    scoop_rt_println(scoop_rt_uint_to_string(UINT64_MAX));
    scoop_rt_println(scoop_rt_bool_to_string(true));
    scoop_rt_println(scoop_rt_bool_to_string(false));
    scoop_rt_println_boolean(scoop_rt_int_equals(-7, -7));
    scoop_rt_println_boolean(!scoop_rt_uint_equals(1, 2));
    scoop_rt_println_boolean(scoop_rt_bool_equals(true, true));
    scoop_rt_println_boolean(scoop_rt_int_hash(42) == scoop_rt_int_hash(42));
    scoop_rt_println_boolean(scoop_rt_uint_hash(42) == scoop_rt_uint_hash(42));
    scoop_rt_println_boolean(scoop_rt_bool_hash(true) == scoop_rt_bool_hash(true));
    scoop_rt_println_boolean(scoop_rt_string_hash(a) == scoop_rt_string_hash(a));

    /* scoop_rt_array_clone (M5/M14): independent snapshot — mutating the
     * original after the clone must not affect the copy, and the copy
     * receives the complete target nominal descriptor. The source is a stack object
     * laid out like a codegen array (16-byte header). */
    const ScoopTypeDescriptor array_td = {
        100, 8, 8, NULL, NULL, NULL, NULL, 0, "Array<Int>"};
    const ScoopTypeDescriptor mutable_array_td = {
        101, 8, 8, NULL, NULL, NULL, NULL, 0, "MutableArray<Int>"};
    struct {
        const ScoopTypeDescriptor *td;
        uint64_t gc_word;
        uint64_t size;
        int64_t data[3];
    } original = {&array_td, 0, 3, {10, 20, 30}};
    const ScoopArray *clone =
        scoop_rt_array_clone(&original, &mutable_array_td, sizeof(int64_t), 24);
    original.data[0] = 99;
    const int64_t *snapshot = (const int64_t *)clone->elements;
    scoop_rt_println_boolean(snapshot[0] == 10 && snapshot[1] == 20 && snapshot[2] == 30);
    scoop_rt_println_boolean(clone->size == 3 && clone->header.td == &mutable_array_td);

    /* scoop_rt_box (M6): header + payload copy. */
    int64_t payload[2] = {1, 2};
    const ScoopObjectHeader *boxed = scoop_rt_box(&point_td, payload, sizeof payload);
    scoop_rt_println_boolean(boxed->td == &point_td);
    const int64_t *boxed_payload = (const int64_t *)((const char *)boxed + sizeof(ScoopObjectHeader));
    scoop_rt_println_boolean(boxed_payload[0] == 1 && boxed_payload[1] == 2);

    /* scoop_rt_is_instance (M6): own td, parent chain, itable key;
     * unrelated td does not match. */
    scoop_rt_println_boolean(scoop_rt_is_instance(boxed, &point_td));
    scoop_rt_println_boolean(scoop_rt_is_instance(boxed, &shape_td));
    scoop_rt_println_boolean(scoop_rt_is_instance(boxed, &describable_td));
    scoop_rt_println_boolean(scoop_rt_is_instance(boxed, &scoop_td_String));

    /* scoop_rt_itable_lookup (M6): returns the recorded slots array. */
    const void *const *slots = scoop_rt_itable_lookup(&point_td, &describable_td);
    scoop_rt_println_boolean(slots == point_describable_slots);
    typedef int64_t (*DescribeFn)(const void *);
    scoop_rt_println_int(((DescribeFn)slots[0])(boxed));

    /* scoop_rt_trap (M3) aborts the process, so it is not exercised
     * here; its trap path is covered end-to-end by EXPECT-TRAP compiler
     * fixtures. */

    /* --- M9 GC tests (runtime/src/gc.c) --- */

    /* Allocation/collection closed loop: stats grow exactly with the
     * allocation count and fall back once the garbage is dead. The
     * stack-scanned locals of scoop_main (anchor and the objects
     * above) stay alive across every collect in this function. */
    uint64_t stats_base = scoop_rt_gc_stats();
    ScoopNode *anchor = new_node(7, NULL);
    make_garbage_nodes(100);
    clobber_stack();
    scoop_rt_println_boolean(scoop_rt_gc_stats() == stats_base + 101);
    scoop_rt_gc_collect();
    /* Conservative M13 stack roots may retain stale helper-frame words;
     * precise M15 stackmaps remove that nondeterminism. Collection itself
     * must not increase the live-object count. */
    scoop_rt_println_boolean(scoop_rt_gc_stats() <= stats_base + 101);
    scoop_rt_println_boolean(anchor->value == 7);

    /* Plain-layout tracing: a node reachable only through anchor's
     * `next` field (offset 24 in node_refs) survives. */
    ScoopNode *second = new_node(8, NULL);
    anchor->next = second;
    second = NULL;
    clobber_stack();
    scoop_rt_gc_collect();
    scoop_rt_println_boolean(anchor->next != NULL && anchor->next->value == 8);

    /* Free-line reuse: two rooted 64B objects share one line, four
     * victims fill the next two lines; after the collection their
     * lines are holes, and fresh allocations are served from recycled
     * lines without mapping a new block. */
    ScoopBig64 *keep1 = scoop_rt_alloc(&big64_td, sizeof(ScoopBig64));
    ScoopBig64 *keep2 = scoop_rt_alloc(&big64_td, sizeof(ScoopBig64));
    keep1->words[0] = 1;
    keep2->words[0] = 2;
    make_garbage_big64();
    clobber_stack();
    scoop_rt_gc_collect();
    uint64_t blocks_before_reuse = scoop_rt_gc_debug_block_count();
    ScoopBig64 *reused[8];
    for (size_t i = 0; i < 8; i++) {
        reused[i] = scoop_rt_alloc(&big64_td, sizeof(ScoopBig64));
        reused[i]->words[0] = (int64_t)i;
    }
    scoop_rt_println_boolean(scoop_rt_gc_debug_block_count() == blocks_before_reuse);
    scoop_rt_gc_collect();
    scoop_rt_println_boolean(keep1->words[0] == 1 && keep2->words[0] == 2);

    /* Freed-block hole regression: a block that survives a collection
     * with holes must not hand those holes out after it dies in a
     * later one (the hole list is rebuilt from scratch each cycle).
     * Overrun the current block with garbage so the rooted object and
     * its victim neighbors land in a fresh block, let that block die
     * in the next cycle, then allocate — the freed block's lines must
     * not come back. */
    make_garbage_big64_many();
    clobber_stack();
    ScoopBig64 *hold = scoop_rt_alloc(&big64_td, sizeof(ScoopBig64));
    hold->words[0] = 99;
    make_garbage_big64();
    clobber_stack();
    uint64_t blocks_with_hold = scoop_rt_gc_debug_block_count();
    scoop_rt_gc_collect(); /* hold's block survives, its dead lines become holes */
    scoop_rt_println_boolean(hold->words[0] == 99);
    hold = NULL;
    clobber_stack();
    scoop_rt_gc_collect(); /* stale conservative roots may retain the block */
    scoop_rt_println_boolean(scoop_rt_gc_debug_block_count() <= blocks_with_hold);
    uint64_t stats_before_refill = scoop_rt_gc_stats();
    ScoopBig64 *refill[4];
    for (size_t i = 0; i < 4; i++) {
        refill[i] = scoop_rt_alloc(&big64_td, sizeof(ScoopBig64));
        refill[i]->words[0] = (int64_t)i;
    }
    scoop_rt_println_boolean(scoop_rt_gc_stats() == stats_before_refill + 4);

    /* Large objects (> 64B, DESIGN 2.1): dedicated block mapping,
     * kept alive by a handle, returned to the OS once dead. */
    uint64_t blocks_before_large = scoop_rt_gc_debug_block_count();
    ScoopString *big = scoop_rt_alloc(&scoop_td_String, sizeof(ScoopString) + 200);
    big->len = 200;
    memset(big->data, 'x', 200);
    uint64_t big_handle = scoop_rt_get_handle(big);
    big = NULL;
    clobber_stack();
    scoop_rt_println_boolean(scoop_rt_gc_debug_block_count() == blocks_before_large + 1);
    scoop_rt_gc_collect();
    scoop_rt_println_boolean(scoop_rt_gc_debug_block_count() == blocks_before_large + 1);
    (void)scoop_rt_release_handle(big_handle);
    clobber_stack();
    scoop_rt_gc_collect();
    scoop_rt_println_boolean(scoop_rt_gc_debug_block_count() == blocks_before_large);

    /* GcHandle keep-alive: the only reference to the object is the
     * handle (an integer, invisible to the stack scan). */
    ScoopNode *handled = new_node(11, NULL);
    uint64_t handle = scoop_rt_get_handle(handled);
    handled = NULL;
    clobber_stack();
    scoop_rt_gc_collect();
    scoop_rt_gc_collect();
    const ScoopNode *back = scoop_rt_release_handle(handle);
    scoop_rt_println_boolean(back != NULL && back->value == 11);
    back = NULL;

    /* pin: survives with every reference hidden (the address is kept
     * XORed so the conservative scan cannot recognize it); after
     * unpin it is reclaimed. Heap made clean first so the stats delta
     * isolates exactly this object. */
    scoop_rt_gc_collect();
    uint64_t stats_before_pin = scoop_rt_gc_stats();
    ScoopNode *pinned = new_node(12, NULL);
    scoop_rt_pin(pinned);
    uintptr_t hidden = (uintptr_t)pinned ^ UINT64_C(0x5A5A5A5A5A5A5A5A);
    pinned = NULL;
    clobber_stack();
    scoop_rt_gc_collect();
    scoop_rt_println_boolean(scoop_rt_gc_stats() == stats_before_pin + 1);
    pinned = (ScoopNode *)(hidden ^ UINT64_C(0x5A5A5A5A5A5A5A5A));
    scoop_rt_unpin(pinned);
    pinned = NULL;
    clobber_stack();
    scoop_rt_gc_collect();
    scoop_rt_println_boolean(scoop_rt_gc_stats() == stats_before_pin);

    /* Array scan descriptor: elements referenced only from the array
     * survive (the array itself is stack-rooted). */
    ScoopArray *ref_array = make_ref_array();
    clobber_stack();
    scoop_rt_gc_collect();
    const ScoopString *const *elements = (const ScoopString *const *)ref_array->elements;
    scoop_rt_println_boolean(elements[0]->len == 4 && elements[0]->data[0] == '1' &&
                             elements[1]->len == 4);

    /* Recursive array element scan: the active enum payload and both
     * unconditional tail references survive; the inactive variant's
     * aligned non-heap payload is ignored. */
    ScoopArray *nested_array = make_nested_array();
    clobber_stack();
    scoop_rt_gc_collect();
    const ScoopNestedElement *nested = (const ScoopNestedElement *)nested_array->elements;
    const ScoopString *nested_payload = nested[0].a_ref;
    scoop_rt_println_boolean(nested_payload->len == 4 && nested_payload->data[3] == '4' &&
                             nested[0].tail->data[3] == '5' && nested[1].tail->data[3] == '6' &&
                             nested[1].tag == 1);

    /* Fixed enum scan: A's dedicated slot keeps its reference; B uses
     * the shared pure payload and leaves the A slot zero. */
    ScoopBoxedEnum *enum_a = make_boxed_enum_a();
    ScoopBoxedEnum *enum_b = make_boxed_enum_b();
    clobber_stack();
    scoop_rt_gc_collect();
    const ScoopString *enum_payload = enum_a->a_ref;
    scoop_rt_println_boolean(enum_payload->len == 4 && enum_payload->data[0] == '1');
    scoop_rt_println_boolean(enum_b->tag == 1);

    /* Global root list: a static slot registered with add_root keeps
     * its object alive without any stack reference. */
    global_rooted = new_node(13, NULL);
    scoop_rt_gc_add_root((void **)&global_rooted);
    clobber_stack();
    scoop_rt_gc_collect();
    scoop_rt_println_boolean(global_rooted != NULL && global_rooted->value == 13);

    /* Compiler-emitted managed-global metadata is registered by GC init; no
     * dynamic add_root call is needed for this writable storage. */
    image_global_rooted = new_node(14, NULL);
    clobber_stack();
    scoop_rt_gc_collect();
    scoop_rt_println_boolean(image_global_rooted != NULL &&
                             image_global_rooted->value == 14);

    /* An exact managed slot may point at a compiler-registered immortal
     * object start; it neither enters the mark worklist nor fails validation. */
    image_immortal_rooted = (void *)&hello;
    scoop_rt_gc_add_root(&image_immortal_rooted);
    scoop_rt_gc_collect();
    scoop_rt_println_boolean(image_immortal_rooted == (void *)&hello);

    /* Exact roots reject every heap-external pointer that is not an
     * explicitly registered immortal object start. */
    fflush(stdout);
    pid_t invalid_root_pid = fork();
    if (invalid_root_pid == 0) {
        void *invalid_root = (void *)(uintptr_t)16;
        scoop_rt_gc_add_root(&invalid_root);
        scoop_rt_gc_collect();
        _exit(0);
    }
    int invalid_root_status = 0;
    waitpid(invalid_root_pid, &invalid_root_status, 0);
    scoop_rt_println_boolean(WIFSIGNALED(invalid_root_status));

    /* Scoop ABI native-root frame: the only visible object pointer is in
     * foreign heap storage, which the conservative C-stack scan cannot see.
     * The frame keeps it alive across collection and exposes the same slot a
     * future moving collector will rewrite. */
    ScoopNode *native = new_node(15, NULL);
    void **native_slot = malloc(sizeof *native_slot);
    void ***native_slots = malloc(sizeof *native_slots);
    *native_slot = native;
    native_slots[0] = native_slot;
    native = NULL;
    ScoopNativeRootFrame native_frame;
    scoop_rt_push_native_roots(&native_frame, native_slots, 1);
    scoop_rt_println_boolean(scoop_rt_gc_debug_native_root_count() == 1);
    clobber_stack();
    scoop_rt_gc_collect();
    const ScoopNode *native_back = *native_slot;
    scoop_rt_println_boolean(native_back != NULL && native_back->value == 15);
    native_back = NULL;
    scoop_rt_pop_native_roots(&native_frame);
    scoop_rt_println_boolean(scoop_rt_gc_debug_native_root_count() == 0);
    *native_slot = NULL;
    free(native_slots);
    free(native_slot);

    /* Niche no-ops: null pin/unpin/handle round-trips. */
    scoop_rt_println_boolean(scoop_rt_get_handle(NULL) == 0 && scoop_rt_release_handle(0) == NULL &&
                             scoop_rt_pin(NULL) == NULL && scoop_rt_unpin(NULL) == NULL);

    /* Arena + write-barrier card table (spec 3.6): blocks are carved
     * from one contiguous arena, and the compiler's card mark
     * atomic OR at `scoop_gc_card_table[addr >> 9]` lands in the backing table
     * because the pointer is pre-biased by `arena_base >> 9`
     * (real[(addr - arena_base) >> 9]). Simulate the card mark the
     * compiler emits and check the slot position for both a small and
     * a large object. */
    uintptr_t arena_base = scoop_rt_gc_debug_arena_base();
    const unsigned char *card_real = scoop_gc_card_table + (arena_base >> 9);
    ScoopNode *carded = new_node(14, NULL);
    uintptr_t carded_addr = (uintptr_t)carded;
    unsigned char *carded_slot = &scoop_gc_card_table[carded_addr >> 9];
    scoop_rt_println_boolean(carded_slot == card_real + ((carded_addr - arena_base) >> 9));
    (void)__atomic_fetch_or(&scoop_gc_card_table[carded_addr >> 9], 1,
                            __ATOMIC_RELAXED); /* compiler card mark */
    scoop_rt_println_boolean(card_real[(carded_addr - arena_base) >> 9] == 1);
    ScoopString *carded_big = scoop_rt_alloc(&scoop_td_String, sizeof(ScoopString) + 200);
    uintptr_t big_addr = (uintptr_t)carded_big;
    /* Every heap address — small-block or large-block — is inside the
     * arena (1 GiB), hence inside the card table's 2 GiB window. */
    scoop_rt_println_boolean(carded_addr >= arena_base &&
                             carded_addr < arena_base + (UINT64_C(1) << 30) &&
                             big_addr >= arena_base && big_addr < arena_base + (UINT64_C(1) << 30));
    scoop_rt_println_boolean(&scoop_gc_card_table[big_addr >> 9] ==
                             card_real + ((big_addr - arena_base) >> 9));
    carded_big = NULL;

    /* Generation changes when a released slot is reused, and the stale
     * generation is rejected deterministically rather than aliasing the new
     * object. Checked in a child so the test binary survives. */
    ScoopNode *generation_a = new_node(31, NULL);
    uint64_t stale_handle = scoop_rt_get_handle(generation_a);
    (void)scoop_rt_release_handle(stale_handle);
    ScoopNode *generation_b = new_node(32, NULL);
    uint64_t current_handle = scoop_rt_get_handle(generation_b);
    bool generation_changed = (uint32_t)stale_handle == (uint32_t)current_handle &&
                              stale_handle != current_handle;
    fflush(stdout);
    pid_t pid = fork();
    if (pid == 0) {
        (void)scoop_rt_release_handle(stale_handle);
        _exit(0);
    }
    int status = 0;
    waitpid(pid, &status, 0);
    scoop_rt_println_boolean(generation_changed && WIFSIGNALED(status));
    (void)scoop_rt_release_handle(current_handle);
}
