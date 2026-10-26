/* Checked representation operations over a stable managed object. Callers
 * validate the expected shape first and do not safepoint before reading it. */
#include "../gc/gc_internal.h"
#include "../value_shape.h"

void scoop_gc_check_unbox(const void *object, const ScoopTypeDescriptor *expected,
                          uint32_t storage_kind) {
    scoop_gc_heap_lock();
    size_t size = scoop_gc_object_size_locked(object);
    const ScoopTypeDescriptor *actual = ((const ScoopObjectHeader *)object)->td;
    if (actual != expected &&
        (storage_kind != SCOOP_INLINE_STORAGE_INLINE_V1 || actual->parent != expected)) {
        scoop_shape_fatal("unbox exact TypeDescriptor mismatch");
    }
    scoop_shape_validate_object(object, size);
    scoop_gc_heap_unlock();
}
