/* Scoop runtime: object model and runtime entry points.
 *
 * See docs/specs/SCOOP-RUNTIME-SPEC.md section 2 (object model) and
 * docs/milestone2/DESIGN.md section 3 (M2 runtime additions).
 */
#ifndef SCOOP_RT_H
#define SCOOP_RT_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/* Runtime spec 2.2. Field set will grow with the GC (parent types,
 * vtable/itable); the M1 prefix matches the final layout. */
typedef struct ScoopTypeDescriptor {
    uint64_t type_id;
    uint64_t size;
    uint64_t align;
    const uint64_t *ref_offsets; /* M1: null */
} ScoopTypeDescriptor;

/* Runtime spec 2.1. */
typedef struct ScoopObjectHeader {
    const ScoopTypeDescriptor *td;
} ScoopObjectHeader;

/* Runtime spec 2.4. Codegen emits string literals as global constants in
 * exactly this layout. */
typedef struct ScoopString {
    ScoopObjectHeader header;
    uint64_t len;
    char data[];
} ScoopString;

/* Runtime spec 2.5 / spec 10.1. Array objects are variable-length:
 * header + element count + inline elements (stride = element layout
 * size, known to codegen; elements of value types are unboxed). */
typedef struct ScoopArray {
    ScoopObjectHeader header;
    uint64_t size;
    char elements[];
} ScoopArray;

/* malloc + write the object header; never freed (always-leak GC,
 * DESIGN 5.1, replaced by Immix in M9). The signature matches the final
 * form so callers do not change. */
void *scoop_rt_alloc(const ScoopTypeDescriptor *td, size_t size);

void scoop_rt_print(const ScoopString *s);
void scoop_rt_println(const ScoopString *s);

/* M2 additions (DESIGN section 3): String concat / structural equality
 * and Int / Boolean builtin output. */
const ScoopString *scoop_rt_string_concat(const ScoopString *a, const ScoopString *b);
bool scoop_rt_string_eq(const ScoopString *a, const ScoopString *b);
void scoop_rt_print_int(int64_t value);
void scoop_rt_println_int(int64_t value);
void scoop_rt_print_boolean(bool value);
void scoop_rt_println_boolean(bool value);

/* M3 addition (DESIGN section 3.1): fatal trap for `!!` on `None`.
 * Writes the message to stderr and aborts; replaced by a real
 * UnwrapException throw in M8. */
_Noreturn void scoop_rt_trap(const char *message);

/* M5 addition (milestone5 DESIGN section 3.1): `Array(m)` /
 * `MutableArray(a)` conversion (spec 10.4). Copies the whole object
 * (header + size + size * elem_size bytes of inline elements) into a
 * fresh allocation — a shallow snapshot: elements that are references
 * are copied as pointers, not cloned. Never freed (always-leak). */
const void *scoop_rt_array_clone(const void *obj, uint64_t elem_size);

/* Entry point provided by the compiled user program (its `fun main`). */
void scoop_main(void);

#endif /* SCOOP_RT_H */
