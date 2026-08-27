/* Scoop runtime (DESIGN section 3). C11. */
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "scoop_rt.h"

/* TypeDescriptor for String, emitted by generated code (runtime spec
 * 2.2). Referenced by scoop_rt_string_concat when allocating. */
extern const ScoopTypeDescriptor scoop_td_String;

void *scoop_rt_alloc(const ScoopTypeDescriptor *td, size_t size) {
    /* Always-leak: memory is never freed (DESIGN 5.1). */
    ScoopObjectHeader *obj = malloc(size);
    if (obj == NULL) {
        fprintf(stderr, "scoop_rt_alloc: out of memory\n");
        abort();
    }
    obj->td = td;
    return obj;
}

void scoop_rt_print(const ScoopString *s) {
    fwrite(s->data, 1, s->len, stdout);
}

void scoop_rt_println(const ScoopString *s) {
    scoop_rt_print(s);
    fputc('\n', stdout);
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
    size_t bytes = sizeof(void *) + sizeof(uint64_t) + (size_t)(src->size * elem_size);
    void *copy = malloc(bytes);
    if (copy == NULL) {
        fprintf(stderr, "scoop_rt_array_clone: out of memory\n");
        abort();
    }
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

int main(void) {
    scoop_main();
    return 0;
}
