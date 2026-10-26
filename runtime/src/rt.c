/* Scoop runtime (DESIGN section 3). C11. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "eh_internal.h"
#include "gc.h"
#include "managed_entries.h"
#include "scoop_rt.h"
#include "thread.h"
#include "value_shape.h"

/* TypeDescriptor for String, emitted by generated code (runtime spec
 * 2.2). Referenced by scoop_rt_string_concat when allocating. */
extern const ScoopTypeDescriptor scoop_td_String;

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

static const ScoopItableEntryV1 *find_itable_entry(
    const ScoopTypeDescriptor *source, const ScoopTypeDescriptor *interface) {
    for (uint64_t i = 0; i < source->itable_count; i++) {
        if (source->itables[i].interface == interface) {
            return &source->itables[i];
        }
    }
    return NULL;
}

static bool type_is_subtype(const ScoopTypeDescriptor *source,
                            const ScoopTypeDescriptor *target) {
    if (target == NULL || source == target) {
        return true;
    }
    if (source == NULL) {
        return false;
    }
    if (source->relation_kind == 4) {
        return true;
    }
    if (target->relation_kind == 4) {
        return false;
    }
    if (target->relation_kind == 1 || target->relation_kind == 2) {
        while (source != NULL && source->relation_kind != 1 &&
               source->relation_kind != 2) {
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
    if (find_itable_entry(source, target) != NULL) {
        return true;
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
    const ScoopTypeDescriptor *source = ((const ScoopObjectHeader *)obj)->td;
    return type_is_subtype(source, td);
}

const ScoopItableEntryV1 *scoop_rt_itable_find(
    const ScoopTypeDescriptor *obj_td, const ScoopTypeDescriptor *iface_td) {
    return find_itable_entry(obj_td, iface_td);
}

const void *const *scoop_rt_itable_lookup(const ScoopTypeDescriptor *obj_td,
                                        const ScoopTypeDescriptor *iface_td) {
    const ScoopItableEntryV1 *entry = find_itable_entry(obj_td, iface_td);
    if (entry != NULL) {
        return entry->slots;
    }
    fprintf(stderr, "scoop_rt_itable_lookup: no itable entry for the interface\n");
    abort();
}
