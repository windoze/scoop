/* Scoop runtime (DESIGN section 3). C11. */
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "eh_internal.h"
#include "gc/gc_internal.h"
#include "managed_entries.h"
#include "scoop_rt.h"
#include "thread.h"
#include "value_shape.h"

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

// Format a fixed-width Long into a fresh ScoopString (GC-allocated).
const ScoopString *scoop_rt_long_to_string(int64_t v) {
    char buf[24]; // -2^63 needs 20 chars + NUL
    int len = snprintf(buf, sizeof(buf), "%lld", (long long)v);
    ScoopString *result = scoop_rt_alloc(
        &scoop_td_String, scoop_shape_allocation_size(&scoop_td_String, (uint64_t)len));
    result->len = (uint64_t)len;
    memcpy(result->data, buf, (size_t)len);
    return result;
}

const ScoopString *scoop_rt_ulong_to_string(uint64_t v) {
    char buf[24]; // 2^64 - 1 needs 20 chars + NUL
    int len = snprintf(buf, sizeof(buf), "%llu", (unsigned long long)v);
    ScoopString *result = scoop_rt_alloc(
        &scoop_td_String, scoop_shape_allocation_size(&scoop_td_String, (uint64_t)len));
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
    ScoopString *result = scoop_rt_alloc(
        &scoop_td_String, scoop_shape_allocation_size(&scoop_td_String, (uint64_t)len));
    result->len = (uint64_t)len;
    memcpy(result->data, text, len);
    return result;
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

int64_t scoop_rt_long_hash(int64_t v) {
    return (int64_t)scoop_rt_mix_word((uint64_t)v);
}

int64_t scoop_rt_ulong_hash(uint64_t v) {
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

const ScoopString *scoop_rt_string_concat_impl(const ScoopString *a,
                                               const ScoopString *b,
                                               uintptr_t return_pc,
                                               uintptr_t stack_pointer,
                                               uintptr_t frame_pointer) {
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, return_pc, stack_pointer, frame_pointer);
    const ScoopString *rooted_a = a;
    const ScoopString *rooted_b = b;
    void **root_slots[] = {(void **)&rooted_a, (void **)&rooted_b};
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, root_slots, 2);
    uint64_t count = scoop_shape_add(rooted_a->len, rooted_b->len);
    size_t size = scoop_shape_allocation_size(&scoop_td_String, count);
    ScoopString *result = scoop_gc_alloc_internal(&scoop_td_String, size);
    result->len = count;
    memcpy(result->data, rooted_a->data, rooted_a->len);
    memcpy(result->data + rooted_a->len, rooted_b->data, rooted_b->len);
    scoop_rt_pop_native_roots(&roots);
    scoop_thread_pop_managed_anchor(&anchor);
    return result;
}

bool scoop_rt_string_eq(const ScoopString *a, const ScoopString *b) {
    return a->len == b->len && memcmp(a->data, b->data, a->len) == 0;
}

int64_t scoop_rt_string_compare(const ScoopString *a, const ScoopString *b) {
    const uint64_t common = a->len < b->len ? a->len : b->len;
    const int order = memcmp(a->data, b->data, (size_t)common);
    if (order < 0) {
        return -1;
    }
    if (order > 0) {
        return 1;
    }
    return (a->len > b->len) - (a->len < b->len);
}

void scoop_rt_print_long(int64_t value) {
    printf("%" PRId64, value);
}

void scoop_rt_println_long(int64_t value) {
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

static bool type_is_subtype(const ScoopTypeDescriptor *source, const ScoopTypeDescriptor *target) {
    if (target == NULL || source == target) {
        return true;
    }
    if (source == NULL) {
        return false;
    }
    if (target->relation_kind == 1 || target->relation_kind == 2) {
        while (source != NULL && source->relation_kind != 1 && source->relation_kind != 2) {
            source = source->parent;
        }
        if (source == NULL || source->relation_kind != target->relation_kind ||
            source->related_type_count != target->related_type_count) {
            return false;
        }
        for (uint32_t i = 0; i < source->related_type_count; i++) {
            if (!type_is_subtype(target->related_types[i], source->related_types[i])) {
                return false;
            }
        }
        return type_is_subtype(source->function_result, target->function_result);
    }
    for (const ScoopTypeDescriptor *cur = source; cur != NULL; cur = cur->parent) {
        if (cur == target) {
            return true;
        }
    }
    for (uint64_t i = 0; i < source->itable_count; i++) {
        if (source->itables[i].interface == target) {
            return true;
        }
    }
    if (source->relation_kind == 3) {
        for (uint32_t i = 0; i < source->related_type_count; i++) {
            if (type_is_subtype(source->related_types[i], target)) {
                return true;
            }
        }
    }
    return false;
}

bool scoop_rt_is_instance(const void *obj, const ScoopTypeDescriptor *td) {
    return type_is_subtype(((const ScoopObjectHeader *)obj)->td, td);
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

int main(void) {
    scoop_thread_runtime_init();
    scoop_callback_runtime_init();
    scoop_rt_gc_init();
    /* The direct C gateway frame is the exclusive upper bound of the managed
     * segment. Runtime sources are built with frame pointers enabled, so the
     * generated frame chain reaches this exact address independently of C
     * local-variable placement. */
    scoop_thread_attach_main();
    scoop_thread_enter_managed(__builtin_frame_address(0));
    scoop_rt_initialize_image();
    scoop_main();
    scoop_callback_prepare_shutdown();
    scoop_eh_prepare_shutdown();
    scoop_thread_leave_managed();
    scoop_thread_prepare_shutdown();
    scoop_thread_detach_main();
    scoop_thread_runtime_finish_shutdown();
    return 0;
}
