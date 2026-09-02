/* Scoop runtime (DESIGN section 3). C11. */
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unwind.h>

#include "scoop_rt.h"
#include "gc/gc_internal.h"
#include "managed_entries.h"
#include "thread.h"

/* M8 (milestone8 DESIGN section 4): exception support on top of the
 * Itanium C++ ABI (runtime spec 5). The __cxa_* entry points and the
 * C++ personality come from the C++ ABI library (-lc++abi). */
extern void *__cxa_allocate_exception(size_t thrown_size);
extern _Noreturn void __cxa_throw(void *thrown_exception, void *tinfo, void (*dest)(void *));
extern _Noreturn void __cxa_rethrow(void);
extern void *__cxa_current_primary_exception(void);
extern _Unwind_Reason_Code __gxx_personality_v0(int version, _Unwind_Action actions,
                                                _Unwind_Exception_Class exception_class,
                                                struct _Unwind_Exception *exception,
                                                struct _Unwind_Context *context);
/* std::set_terminate(void (*)()) — Itanium mangling, also from the C++
 * ABI library. */
extern void _ZSt13set_terminatePFvvE(void (*handler)(void));

/* TypeDescriptor for String, emitted by generated code (runtime spec
 * 2.2). Referenced by scoop_rt_string_concat when allocating. */
extern const ScoopTypeDescriptor scoop_td_String;

/* scoop_rt_alloc lives in gc.c (M9): it allocates from the GC heap and
 * may trigger a collection. */

void scoop_rt_write(const ScoopString *s) {
    fwrite(s->data, 1, s->len, stdout);
}

void scoop_rt_println(const ScoopString *s) {
    scoop_rt_write(s);
    fputc('\n', stdout);
}

// Format an i64 into a fresh ScoopString (GC-allocated), backing
// core's `intToString` (M7).
const ScoopString *scoop_rt_int_to_string(int64_t v) {
    char buf[24]; // -2^63 needs 20 chars + NUL
    int len = snprintf(buf, sizeof(buf), "%lld", (long long)v);
    ScoopString *result = scoop_rt_alloc(&scoop_td_String, sizeof(ScoopString) + (size_t)len);
    result->len = (uint64_t)len;
    memcpy(result->data, buf, (size_t)len);
    return result;
}

const ScoopString *scoop_rt_uint_to_string(uint64_t v) {
    char buf[24]; // 2^64 - 1 needs 20 chars + NUL
    int len = snprintf(buf, sizeof(buf), "%llu", (unsigned long long)v);
    ScoopString *result = scoop_rt_alloc(&scoop_td_String, sizeof(ScoopString) + (size_t)len);
    result->len = (uint64_t)len;
    memcpy(result->data, buf, (size_t)len);
    return result;
}

// "true" / "false" as a fresh ScoopString, backing core's
// `boolToString` (M7).
const ScoopString *scoop_rt_bool_to_string(bool v) {
    static const char TRUE_STR[] = "true";
    static const char FALSE_STR[] = "false";
    const char *text = v ? TRUE_STR : FALSE_STR;
    size_t len = v ? sizeof(TRUE_STR) - 1 : sizeof(FALSE_STR) - 1;
    ScoopString *result = scoop_rt_alloc(&scoop_td_String, sizeof(ScoopString) + len);
    result->len = (uint64_t)len;
    memcpy(result->data, text, len);
    return result;
}

bool scoop_rt_int_equals(int64_t left, int64_t right) {
    return left == right;
}

bool scoop_rt_uint_equals(uint64_t left, uint64_t right) {
    return left == right;
}

bool scoop_rt_bool_equals(bool left, bool right) {
    return left == right;
}

static uint64_t scoop_rt_mix_word(uint64_t value) {
    value ^= value >> 30;
    value *= UINT64_C(0xbf58476d1ce4e5b9);
    value ^= value >> 27;
    value *= UINT64_C(0x94d049bb133111eb);
    value ^= value >> 31;
    return value;
}

int64_t scoop_rt_int_hash(int64_t v) {
    return (int64_t)scoop_rt_mix_word((uint64_t)v);
}

int64_t scoop_rt_uint_hash(uint64_t v) {
    return (int64_t)scoop_rt_mix_word(v);
}

int64_t scoop_rt_bool_hash(bool v) {
    return v ? 1 : 0;
}

int64_t scoop_rt_string_hash(const ScoopString *s) {
    uint64_t hash = UINT64_C(1469598103934665603);
    for (uint64_t index = 0; index < s->len; index++) {
        hash ^= (uint8_t)s->data[index];
        hash *= UINT64_C(1099511628211);
    }
    return (int64_t)hash;
}

const ScoopString *scoop_rt_string_concat_impl(
    const ScoopString *a, const ScoopString *b, uintptr_t return_pc,
    uintptr_t stack_pointer, uintptr_t frame_pointer) {
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, return_pc, stack_pointer,
                                     frame_pointer);
    const ScoopString *rooted_a = a;
    const ScoopString *rooted_b = b;
    void **root_slots[] = {(void **)&rooted_a, (void **)&rooted_b};
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, root_slots, 2);
    ScoopString *result =
        scoop_gc_alloc_internal(&scoop_td_String,
                                sizeof(ScoopString) + rooted_a->len + rooted_b->len);
    result->len = rooted_a->len + rooted_b->len;
    memcpy(result->data, rooted_a->data, rooted_a->len);
    memcpy(result->data + rooted_a->len, rooted_b->data, rooted_b->len);
    scoop_rt_pop_native_roots(&roots);
    scoop_thread_pop_managed_anchor(&anchor);
    return result;
}

bool scoop_rt_string_eq(const ScoopString *a, const ScoopString *b) {
    return a->len == b->len && memcmp(a->data, b->data, a->len) == 0;
}

void scoop_rt_print_int(int64_t value) {
    printf("%" PRId64, value);
}

void scoop_rt_println_int(int64_t value) {
    printf("%" PRId64 "\n", value);
}

void scoop_rt_print_boolean(bool value) {
    fputs(value ? "true" : "false", stdout);
}

void scoop_rt_println_boolean(bool value) {
    scoop_rt_print_boolean(value);
    fputc('\n', stdout);
}

_Noreturn void scoop_rt_trap(const char *message) {
    fprintf(stderr, "scoop: trap: %s\n", message);
    abort();
}

const void *scoop_rt_array_clone_impl(const void *obj,
                                      const ScoopTypeDescriptor *target_td,
                                      uint64_t elem_size,
                                      uint64_t data_offset,
                                      uintptr_t return_pc,
                                      uintptr_t stack_pointer,
                                      uintptr_t frame_pointer) {
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, return_pc, stack_pointer,
                                     frame_pointer);
    const ScoopArray *src = obj;
    void **root_slots[] = {(void **)&src};
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, root_slots, 1);
    size_t bytes = (size_t)data_offset + (size_t)(src->size * elem_size);
    /* The target nominal array application is fixed by MIR/LIR. Preserve the
     * fresh GC header and copy only size/padding/elements. */
    void *copy = scoop_gc_alloc_internal(target_td, bytes);
    memcpy((char *)copy + sizeof(ScoopObjectHeader),
           (const char *)src + sizeof(ScoopObjectHeader),
           bytes - sizeof(ScoopObjectHeader));
    scoop_rt_pop_native_roots(&roots);
    scoop_thread_pop_managed_anchor(&anchor);
    return copy;
}

void *scoop_rt_box_impl(const ScoopTypeDescriptor *td, const void *payload,
                        uint64_t payload_size, const uint64_t *payload_scan,
                        uintptr_t return_pc, uintptr_t stack_pointer,
                        uintptr_t frame_pointer) {
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, return_pc, stack_pointer,
                                     frame_pointer);
    ScoopNativeRegionRootEntry payload_entry;
    ScoopNativeRegionRootFrame payload_roots;
    if (payload_scan != NULL) {
        payload_entry = (ScoopNativeRegionRootEntry){
            .base = (void *)payload,
            .scan = payload_scan,
        };
        scoop_rt_push_native_region_roots(&payload_roots, &payload_entry, 1);
    }
    void *obj = scoop_gc_alloc_internal(
        td, sizeof(ScoopObjectHeader) + (size_t)payload_size);
    memcpy((char *)obj + sizeof(ScoopObjectHeader), payload, (size_t)payload_size);
    if (payload_scan != NULL) {
        scoop_rt_pop_native_region_roots(&payload_roots);
    }
    scoop_thread_pop_managed_anchor(&anchor);
    return obj;
}

bool scoop_rt_is_instance(const void *obj, const ScoopTypeDescriptor *td) {
    const ScoopTypeDescriptor *obj_td = ((const ScoopObjectHeader *)obj)->td;
    for (const ScoopTypeDescriptor *cur = obj_td; cur != NULL; cur = cur->parent) {
        if (cur == td) {
            return true;
        }
    }
    for (uint64_t i = 0; i < obj_td->itable_count; i++) {
        if (obj_td->itables[i].interface == td) {
            return true;
        }
    }
    return false;
}

const void *const *scoop_rt_itable_lookup(const ScoopTypeDescriptor *obj_td,
                                          const ScoopTypeDescriptor *iface_td) {
    for (uint64_t i = 0; i < obj_td->itable_count; i++) {
        if (obj_td->itables[i].interface == iface_td) {
            return obj_td->itables[i].slots;
        }
    }
    fprintf(stderr, "scoop_rt_itable_lookup: no itable entry for the interface\n");
    abort();
}

/* M8 (milestone8 DESIGN section 4, runtime spec 5). */

static void scoop_exception_destructor(void *buffer) {
    scoop_rt_gc_remove_root_object(buffer);
}

_Noreturn void scoop_rt_throw(const void *obj) {
    /* Copy the object into an ABI exception buffer (Itanium ABI
     * usage): __cxa_throw writes the exception header immediately
     * before the thrown pointer, so user objects must never be thrown
     * in place. The buffer from __cxa_allocate_exception carries that
     * headroom; catch handlers observe the buffer pointer — the copy
     * is the exception object (throw-by-value identity, as in C++),
     * and its copied object header keeps the TD available for catch
     * type filtering. The C++ ABI releases the buffer when handling
     * completes. The destructor removes the external GC root; payload
     * references themselves remain GC-managed.
     *
     * M9 keep-alive (DESIGN 3.4): the buffer is not GC-managed, but
     * the references inside the copy must stay alive while the
     * exception is in flight. Register the copied buffer as an
     * external object root so its outgoing references are traced. No
     * GC allocation occurs between the copy and registration, and the
     * original need not stay pinned after that point. */
    const ScoopObjectHeader *header = obj;
    size_t size = (size_t)header->td->size;
    void *buffer = __cxa_allocate_exception(size);
    memcpy(buffer, obj, size);
    scoop_rt_gc_add_root_object(buffer);
    __cxa_throw(buffer, NULL, scoop_exception_destructor);
}

_Noreturn void scoop_rt_rethrow(void) {
    /* Contract (LIR): only called on the no-catch-matched path of a
     * landing pad, i.e. while handling an exception. */
    __cxa_rethrow();
}

void *scoop_rt_materialize_exception_impl(const void *caught,
                                          uintptr_t return_pc,
                                          uintptr_t stack_pointer,
                                          uintptr_t frame_pointer) {
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, return_pc, stack_pointer,
                                     frame_pointer);
    const ScoopObjectHeader *source = caught;
    size_t size = (size_t)source->td->size;
    void *managed = scoop_gc_alloc_internal(source->td, size);
    if (size > sizeof(ScoopObjectHeader)) {
        memcpy((char *)managed + sizeof(ScoopObjectHeader),
               (const char *)caught + sizeof(ScoopObjectHeader),
               size - sizeof(ScoopObjectHeader));
    }
    scoop_thread_pop_managed_anchor(&anchor);
    return managed;
}

int scoop_eh_personality(int version, unsigned int actions, unsigned long long exception_class,
                         void *exception, void *context) {
    /* Minimal personality (runtime spec 5 leaves the mechanism to the
     * implementation): delegate to the C++ ABI personality. Scoop
     * landing pads are catch-all and Scoop exceptions carry a NULL
     * type_info, which __gxx_personality_v0 matches only against
     * catch-all clauses — exactly the Scoop semantics. */
    return (int)__gxx_personality_v0(version, (_Unwind_Action)actions,
                                     (_Unwind_Exception_Class)exception_class,
                                     (struct _Unwind_Exception *)exception,
                                     (struct _Unwind_Context *)context);
}

/* M8 uncaught-exception handling (milestone8 DESIGN 5.2): obtain the
 * active ABI exception buffer, then read the concrete Scoop type name
 * from its object header. */
static _Noreturn void scoop_uncaught_terminate(void) {
    const ScoopObjectHeader *exception = __cxa_current_primary_exception();
    const char *name = "<unknown>";
    if (exception != NULL && exception->td != NULL && exception->td->name != NULL) {
        name = exception->td->name;
    }
    fprintf(stderr, "scoop: uncaught exception: %s\n", name);
    abort();
}

void scoop_rt_init_eh(void) {
    _ZSt13set_terminatePFvvE(scoop_uncaught_terminate);
}

int main(void) {
    scoop_rt_init_eh();
    scoop_thread_runtime_init();
    scoop_callback_runtime_init();
    scoop_rt_gc_init();
    /* The direct C gateway frame is the exclusive upper bound of the managed
     * segment. Runtime sources are built with frame pointers enabled, so the
     * generated frame chain reaches this exact address independently of C
     * local-variable placement. */
    scoop_thread_attach_main(__builtin_frame_address(0));
    scoop_main();
    scoop_callback_prepare_shutdown();
    scoop_thread_prepare_shutdown();
    scoop_thread_detach_main();
    scoop_thread_runtime_finish_shutdown();
    return 0;
}
