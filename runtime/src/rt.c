/* Scoop runtime (DESIGN section 3). C11. */
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unwind.h>

#include "scoop_rt.h"

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

void scoop_rt_print(const ScoopString *s) {
    fwrite(s->data, 1, s->len, stdout);
}

void scoop_rt_println(const ScoopString *s) {
    scoop_rt_print(s);
    fputc('\n', stdout);
}

// Identity for String's own `toString` vtable slot (M7).
const ScoopString *scoop_rt_string_identity(const ScoopString *s) {
    return s;
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

const ScoopString *scoop_rt_string_concat(const ScoopString *a, const ScoopString *b) {
    ScoopString *result =
        scoop_rt_alloc(&scoop_td_String, sizeof(ScoopString) + a->len + b->len);
    result->len = a->len + b->len;
    memcpy(result->data, a->data, a->len);
    memcpy(result->data + a->len, b->data, b->len);
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

const void *scoop_rt_array_clone(const void *obj, uint64_t elem_size) {
    const ScoopArray *src = obj;
    size_t bytes = sizeof(ScoopObjectHeader) + sizeof(uint64_t) + (size_t)(src->size * elem_size);
    /* GC allocation: the copy keeps the source's TypeDescriptor (the
     * conversion preserves the array type, spec 10.4). */
    void *copy = scoop_rt_alloc(src->header.td, bytes);
    memcpy(copy, obj, bytes);
    return copy;
}

void *scoop_rt_box(const ScoopTypeDescriptor *td, const void *payload, uint64_t payload_size) {
    void *obj = scoop_rt_alloc(td, sizeof(ScoopObjectHeader) + (size_t)payload_size);
    memcpy((char *)obj + sizeof(ScoopObjectHeader), payload, (size_t)payload_size);
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

bool scoop_rt_any_equals(const void *a, const void *b) {
    return a == b;
}

uint64_t scoop_rt_any_hashcode(const void *a) {
    return (uint64_t)(uintptr_t)a;
}

const ScoopString *scoop_rt_any_tostring(const void *a) {
    /* "Object@<hex>" minimal form (milestone6 DESIGN section 3). */
    char buf[32];
    int len = snprintf(buf, sizeof buf, "Object@%llx", (unsigned long long)(uintptr_t)a);
    ScoopString *result = scoop_rt_alloc(&scoop_td_String, sizeof(ScoopString) + (size_t)len);
    result->len = (uint64_t)len;
    memcpy(result->data, buf, (size_t)len);
    return result;
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
    /* Record the mutator stack base for the v1 conservative stack
     * scan (gc.c); replaced by statepoint stackmaps once codegen
     * emits them (milestone9 DESIGN 3.2). */
    scoop_rt_gc_init(__builtin_frame_address(0));
    scoop_main();
    return 0;
}
