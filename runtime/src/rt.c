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

int main(void) {
    scoop_main();
    return 0;
}
