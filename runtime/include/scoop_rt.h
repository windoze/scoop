/* Scoop runtime: object model and runtime entry points (M1 minimal set).
 *
 * See docs/specs/SCOOP-RUNTIME-SPEC.md section 2 (object model) and
 * docs/milestone1/DESIGN.md section 3 (M1 runtime).
 */
#ifndef SCOOP_RT_H
#define SCOOP_RT_H

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

/* malloc + write the object header; never freed (always-leak GC,
 * DESIGN 5.1, replaced by Immix in M9). The signature matches the final
 * form so callers do not change. */
void *scoop_rt_alloc(const ScoopTypeDescriptor *td, size_t size);

void scoop_rt_print(const ScoopString *s);
void scoop_rt_println(const ScoopString *s);

/* Entry point provided by the compiled user program (its `fun main`). */
void scoop_main(void);

#endif /* SCOOP_RT_H */
