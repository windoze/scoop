/* Scoop runtime: object model and runtime entry points.
 *
 * See docs/specs/SCOOP-RUNTIME-SPEC.md section 2 (object model) and
 * docs/milestone2/DESIGN.md section 3 (M2 runtime additions). M9
 * (docs/milestone9/DESIGN.md section 2) replaces the always-leak
 * allocator with the Immix-core GC in runtime/src/gc.c and widens the
 * object header to 16 bytes.
 */
#ifndef SCOOP_RT_H
#define SCOOP_RT_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/* Runtime spec 2.2 (full M6 form). Field order is the ABI contract with
 * codegen: it emits one global per LIR meta TypeDescriptor in exactly
 * this layout. */
typedef struct ScoopTypeDescriptor ScoopTypeDescriptor;

/* itable entry: keyed by the interface's TypeDescriptor pointer
 * (runtime spec 2.2, impl spec 2.9). */
typedef struct ScoopItableEntry {
    const ScoopTypeDescriptor *interface;
    const void *const *slots; /* function pointer array */
} ScoopItableEntry;

/* GC scan-descriptor contract (M9 v1; runtime spec 2.2 "引用字段位图/
 * 描述"的具体形态).
 *
 * The GC finds an object's outgoing references purely through
 * `ref_offsets`; `type_id` is not interpreted by the GC. `size` /
 * `align` are not used by the GC either (small objects never straddle
 * lines, large objects own a whole block — see runtime/src/gc.c), so
 * array TDs keep their element-level `size` (scoop_rt_array_clone).
 *
 * `ref_offsets` points to a sequence of u64 words whose first word
 * selects the kind:
 *
 * - NULL: the object has no outgoing references (String, plain objects
 *   without reference fields, arrays of non-reference elements).
 * - SCOOP_REFS_ARRAY: array object. Word 1 is the element stride and
 *   word 2 is a pointer (stored as u64) to the recursive scan program
 *   for one element. The count is at object offset 16; elements start
 *   at offset 24. A no-reference element scan makes the whole array
 *   descriptor NULL.
 * - SCOOP_REFS_ENUM: tagged enum at an arbitrary inline offset. Word 1
 *   is the tag byte offset, word 2 is the variant count N, and words
 *   3 .. 3+N are pointers to recursive per-variant scan programs. This
 *   composes for enums nested in class/struct/tuple fields. Niche enums
 *   are ordinary one-word references and use a plain table.
 * - SCOOP_REFS_SEQUENCE: composition of independent scans over the same
 *   base. Word 1 is child count N and words 2 .. 2+N are pointers to
 *   recursive child programs. This combines unconditional references
 *   with one or more nested tagged enums.
 * - otherwise the word is a count N (< SCOOP_REFS_ENUM) and the
 *   following N words are the object-relative byte offsets of the
 *   reference fields (plain layout).
 *
 * A scanned slot whose value is null or points outside the GC heap is
 * ignored, so null (niche `None`) references and stale bytes are safe.
 */
#define SCOOP_REFS_ARRAY UINT64_MAX
#define SCOOP_REFS_ENUM (UINT64_MAX - 1)
#define SCOOP_REFS_SEQUENCE (UINT64_MAX - 2)

struct ScoopTypeDescriptor {
    uint64_t type_id;
    uint64_t size;
    uint64_t align;
    const uint64_t *ref_offsets; /* scan descriptor (see above) or null */
    const ScoopTypeDescriptor *parent;
    const void *const *vtable; /* function pointer array or null */
    const ScoopItableEntry *itables; /* entry array or null */
    uint64_t itable_count;
    const char *name; /* stable NUL-terminated UTF-8 diagnostic name */
};

/* Runtime spec 2.1, M9 form (milestone9 DESIGN section 0): 16 bytes.
 * `gc_word` belongs to the GC (runtime/src/gc.c): mark parity bit and
 * pin bit; the remaining bits are reserved (hash cache etc.). Mutator
 * code must not touch it. All field payloads start at offset 16. */
typedef struct ScoopObjectHeader {
    const ScoopTypeDescriptor *td;
    uint64_t gc_word;
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

/* Allocate `size` bytes (including the 16-byte header) from the GC heap
 * and write the object header (`td` + zeroed gc_word). May trigger a
 * collection (runtime spec 3.1 slow path). The signature matches the
 * pre-M9 always-leak form so callers do not change. */
void *scoop_rt_alloc(const ScoopTypeDescriptor *td, size_t size);

/* M9 GC contracts (milestone9 DESIGN 3.1): write-barrier card table
 * and the safepoint poll symbol emitted by the compiler.
 *
 * Card table (spec 3.6): generated code marks
 * `scoop_gc_card_table[addr >> 9] = 1` after heap stores. The symbol
 * is a POINTER VARIABLE — load it, then GEP — pre-biased by the
 * runtime with `arena_base >> 9`, so the formula lands in the backing
 * table (4 MiB, covering the 2 GiB window above the arena) as
 * `real[(addr - arena_base) >> 9]`. Blocks are carved from one
 * fixed-address arena (gc.c), so every heap address is in bounds.
 * The barrier is only emitted for heap stores; v1's collector ignores
 * the cards. */
extern unsigned char *scoop_gc_card_table;
void scoop_rt_safepoint(void);

void scoop_rt_print(const ScoopString *s);
void scoop_rt_println(const ScoopString *s);

const ScoopString *scoop_rt_string_identity(const ScoopString *s);

/* M7 additions (milestone7 DESIGN section 3): primitive conversions
 * backing core's intToString / boolToString. */
const ScoopString *scoop_rt_int_to_string(int64_t v);
const ScoopString *scoop_rt_bool_to_string(bool v);

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
 * fresh GC allocation — a shallow snapshot: elements that are
 * references are copied as pointers, not cloned. */
const void *scoop_rt_array_clone(const void *obj, uint64_t elem_size);

/* M6 additions (milestone6 DESIGN section 3): dispatch support. */

/* Box a value type: allocate header + payload and copy the payload
 * (runtime spec 2.3). */
void *scoop_rt_box(const ScoopTypeDescriptor *td, const void *payload, uint64_t payload_size);

/* `is` check: walk the object's parent chain, then scan its itable
 * keys (runtime spec 2.2). */
bool scoop_rt_is_instance(const void *obj, const ScoopTypeDescriptor *td);

/* Linear scan of `obj_td`'s itable keys; returns the slots array.
 * Aborts when missing (the compiler guarantees the entry exists —
 * defensive). */
const void *const *scoop_rt_itable_lookup(const ScoopTypeDescriptor *obj_td,
                                          const ScoopTypeDescriptor *iface_td);

/* Any default methods (vtable slots 0-2 on every class, milestone6
 * DESIGN 5.1). */
bool scoop_rt_any_equals(const void *a, const void *b);
uint64_t scoop_rt_any_hashcode(const void *a);
const ScoopString *scoop_rt_any_tostring(const void *a);

/* M9 additions (milestone9 DESIGN section 2): GC interface. See
 * runtime/src/gc.c for the implementation and docs/specs/
 * SCOOP-RUNTIME-SPEC.md sections 3-4 for the contract. */

/* Record the base (highest address) of the mutator stack; called once
 * from the runtime's main before scoop_main. `stack_base` should be a
 * frame address of main's caller frame. Required only by the v1
 * conservative stack scan (see gc.c); collections before this call
 * simply skip stack scanning. */
void scoop_rt_gc_init(void *stack_base);

/* Register a global root (runtime spec 3.3): `slot` is the address of
 * a variable holding an object pointer (or null); it is re-read at
 * every collection. Values pointing outside the GC heap (e.g. static
 * string literals) are ignored. v1 has no removal API (conservative;
 * roots live as long as the process). */
void scoop_rt_gc_add_root(void **slot);

/* Register an object-like region outside the GC heap as a root: its
 * outgoing references are traced at every collection, but the region
 * itself is neither marked nor reclaimed (used by scoop_rt_throw for
 * the ABI exception buffer, milestone9 DESIGN 3.4). */
void scoop_rt_gc_add_root_object(const void *obj);

/* Remove a previously registered external object root. The exact
 * pointer must be present once; used by the C++ ABI exception
 * destructor when a thrown buffer's lifetime ends. */
void scoop_rt_gc_remove_root_object(const void *obj);

/* pin / unpin (runtime spec 3.4): O(1) object-header flag, no handle
 * table. Returns the object so the Scoop-level intrinsics can forward
 * it. null is a no-op returning null; a non-null pointer that is not a
 * GC-heap object start aborts. Pinning is not ref-counted: one unpin
 * clears any number of pins. */
const void *scoop_rt_pin(const void *obj);
const void *scoop_rt_unpin(const void *obj);

/* GcHandle table (runtime spec 3.4): keeps the object alive without
 * pinning. The handle value is the table index + 1; 0 is reserved for
 * the niche (get_handle(null) == 0, release_handle(0) == null).
 * release_handle validates the handle and aborts on an invalid or
 * already-released one (runtime spec 4.2). Entries are not updated by
 * the collector (v1 does not move objects; a moving collector must
 * update entries instead — noted in gc.c). */
uint64_t scoop_rt_get_handle(const void *obj);
const void *scoop_rt_release_handle(uint64_t handle);

/* Force a full collection. */
void scoop_rt_gc_collect(void);

/* Number of objects currently allocated from the GC heap. Includes
 * not-yet-collected garbage; right after scoop_rt_gc_collect() it is
 * the precise live count. Test/diagnostic hook (milestone9 DESIGN 1:
 * gcStats). */
uint64_t scoop_rt_gc_stats(void);

/* Test hook: number of heap blocks currently live in the arena. */
uint64_t scoop_rt_gc_debug_block_count(void);

/* Test hook: base address of the heap arena the blocks are carved
 * from (0 before the first allocation / gc init). */
uintptr_t scoop_rt_gc_debug_arena_base(void);

/* Test hook: number of registered global/external roots. */
uint64_t scoop_rt_gc_debug_root_count(void);

/* M8 additions (milestone8 DESIGN section 4): exception support on top
 * of the Itanium C++ ABI (runtime spec 5). Scoop exceptions are thrown
 * with a NULL type_info; generated landing pads use a single catch-all
 * (null) clause, and catch type filtering is done by generated code
 * (scoop_rt_is_instance). Linking needs the C++ ABI library
 * (-lc++abi). */

/* Throw `obj` as a Scoop exception; does not return. M9 (DESIGN 3.4):
 * registers the ABI buffer as an external object root; the exception
 * destructor removes it when the ABI lifetime ends. */
_Noreturn void scoop_rt_throw(const void *obj);

/* Rethrow the exception currently being handled; does not return. */
_Noreturn void scoop_rt_rethrow(void);

/* Install the uncaught-exception terminate handler; called once from
 * the runtime's main before scoop_main. */
void scoop_rt_init_eh(void);

/* Personality function set on every generated function that contains a
 * landing pad. The arguments follow the Itanium personality protocol
 * (declared with plain C types to keep this header unwind.h-free); the
 * unwinder always passes all five. */
int scoop_eh_personality(int version, unsigned int actions, unsigned long long exception_class,
                         void *exception, void *context);

/* Entry point provided by the compiled user program (its `fun main`). */
void scoop_main(void);

#endif /* SCOOP_RT_H */
