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
 *   for one element. The count is at object offset 16; scanned managed
 *   element types have alignment at most 8 and therefore start at
 *   offset 24. Over-aligned C-layout elements are GC-free and carry a
 *   NULL descriptor. A no-reference element scan makes the whole array
 *   descriptor NULL.
 * - SCOOP_REFS_SEQUENCE: composition of independent scans over the same
 *   base. Word 1 is child count N and words 2 .. 2+N are pointers to
 *   recursive child programs.
 * - otherwise the word is a count N (< SCOOP_REFS_SEQUENCE) and the
 *   following N words are the object-relative byte offsets of the
 *   reference fields. Tagged enums use this same fixed-offset form:
 *   each ref-bearing variant has a disjoint slot and inactive slots are
 *   zero, so scanning never reads the tag.
 *
 * A scanned slot must contain null, a GC-heap object start, or a registered
 * immortal object start. Any other value is a malformed managed reference
 * and is fatal. Niche `None` and inactive tagged-enum slots must be null.
 */
#define SCOOP_REFS_ARRAY UINT64_MAX
#define SCOOP_REFS_SEQUENCE (UINT64_MAX - 1)

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

/* Runtime spec 2.1: 16 bytes. `gc_word` belongs to the GC and currently
 * carries the pin bit; mark, exact size and forwarding state live in the
 * arena-external side table. The remaining bits are reserved (hash cache
 * etc.). Mutator code must not touch it. All payloads start at offset 16. */
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

/* M15 image root metadata. Codegen emits exactly one instance of each table
 * and count symbol for the linked image. A zero-count table still has one
 * null sentinel record so its symbol is addressable. Runtime initialization
 * validates and registers both tables before any managed code executes. */
typedef struct ScoopManagedGlobalDescriptor {
    void *writable_base;
    const uint64_t *scan;
} ScoopManagedGlobalDescriptor;

typedef struct ScoopImmortalObjectDescriptor {
    const void *object_start;
    uint64_t object_size;
    const ScoopTypeDescriptor *td;
} ScoopImmortalObjectDescriptor;

extern const ScoopManagedGlobalDescriptor scoop_image_managed_globals[];
extern const uint64_t scoop_image_managed_global_count;
extern const ScoopImmortalObjectDescriptor scoop_image_immortal_objects[];
extern const uint64_t scoop_image_immortal_object_count;

/* Runtime spec 2.5 / spec 10.1. Array objects are variable-length:
 * header + element count + padding to the element alignment + inline
 * elements (stride = element layout size, known to codegen; elements
 * of value types are unboxed). `elements` names the minimum-alignment
 * offset; codegen computes the actual aligned data offset. */
typedef struct ScoopArray {
    ScoopObjectHeader header;
    uint64_t size;
    char elements[];
} ScoopArray;

/* Minimal per-thread allocation ABI exposed to generated code. The pointed
 * context belongs to the attached OS thread. Generated code may only read
 * and advance cursor while managed; the STW collector retires both fields
 * before sweep. */
typedef struct ScoopAllocationContext {
    char *cursor;
    char *limit;
} ScoopAllocationContext;

extern _Thread_local ScoopAllocationContext *scoop_rt_allocation_context;

/* Complete one successful inline TLAB allocation. This helper is a GC leaf:
 * it clears the object, initializes its header, atomically records the object
 * start, and updates statistics. */
void scoop_runtime_finish_tlab_alloc(void *object,
                                     const ScoopTypeDescriptor *td,
                                     size_t size);

/* ManagedEntry: refill/allocate after generated code's inline TLAB bump
 * fails. The target anchor stub publishes the direct managed caller before
 * this operation can park or collect. */
void *scoop_runtime_alloc_slow(const ScoopTypeDescriptor *td, size_t size);

/* NativeBorrowedEntry for Scoop-ABI C implementations. Generated code uses
 * the inline allocation context and falls back to the distinct managed entry
 * above. The native caller must already have published caller/native roots. */
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

void scoop_rt_write(const ScoopString *s);
void scoop_rt_println(const ScoopString *s);


/* M7 additions (milestone7 DESIGN section 3): primitive conversions
 * backing core's intToString / boolToString. */
const ScoopString *scoop_rt_int_to_string(int64_t v);
const ScoopString *scoop_rt_uint_to_string(uint64_t v);
const ScoopString *scoop_rt_bool_to_string(bool v);
bool scoop_rt_int_equals(int64_t left, int64_t right);
bool scoop_rt_uint_equals(uint64_t left, uint64_t right);
bool scoop_rt_bool_equals(bool left, bool right);
int64_t scoop_rt_int_hash(int64_t v);
int64_t scoop_rt_uint_hash(uint64_t v);
int64_t scoop_rt_bool_hash(bool v);
int64_t scoop_rt_string_hash(const ScoopString *s);

/* String and primitive operations used by ordinary scoop.core declarations. */
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

/* ManagedEntry for `Array(m)` /
 * `MutableArray(a)` conversion (spec 10.4). Copies the whole object
 * (data_offset + size * elem_size bytes, including header/size/padding)
 * into a fresh GC allocation — a shallow snapshot: elements that are
 * references are copied as pointers, not cloned. */
const void *scoop_rt_array_clone(const void *obj,
                                 const ScoopTypeDescriptor *target_td,
                                 uint64_t elem_size, uint64_t data_offset);

/* M6 additions (milestone6 DESIGN section 3): dispatch support. */

/* ManagedEntry: box an addressable value payload. `payload_scan` is complete,
 * payload-relative metadata emitted by LIR/codegen; null means GC-free. */
void *scoop_rt_box(const ScoopTypeDescriptor *td, const void *payload,
                   uint64_t payload_size, const uint64_t *payload_scan);

/* `is` check: walk the object's parent chain, then scan its itable
 * keys (runtime spec 2.2). */
bool scoop_rt_is_instance(const void *obj, const ScoopTypeDescriptor *td);

/* Linear scan of `obj_td`'s itable keys; returns the slots array.
 * Aborts when missing (the compiler guarantees the entry exists —
 * defensive). */
const void *const *scoop_rt_itable_lookup(const ScoopTypeDescriptor *obj_td,
                                          const ScoopTypeDescriptor *iface_td);

/* M9 additions (milestone9 DESIGN section 2): GC interface. See
 * runtime/src/gc.c for the implementation and docs/specs/
 * SCOOP-RUNTIME-SPEC.md sections 3-4 for the contract. */

/* Initialize the GC heap before the main thread enters Scoop code. Stack
 * bounds and per-thread roots belong to M13's attached thread state rather
 * than to a process-global main-thread variable. */
void scoop_rt_gc_init(void);

/* M13 thread registration baseline (runtime spec 3.5 / 7). A foreign
 * thread must attach before any later managed callback entry. The return
 * value is true only when this call created the attachment; nested users
 * must detach only when they own that true result. A newly attached foreign
 * thread starts in native-safe mode and cannot execute managed code until a
 * callback gateway performs the managed transition. */
bool scoop_rt_attach_foreign_thread(void);
void scoop_rt_detach_foreign_thread(void);

/* Register a process-lifetime writable root slot (runtime spec 3.3). The
 * collector re-reads and rewrites it at every collection. Its value must be
 * null, a current GC object start, or an exactly registered stable object;
 * every other address is fatal. There is deliberately no removal API. */
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

/* M12 Scoop-ABI native roots (runtime spec 4.2). A native function that
 * keeps direct managed references across an allocation/collection stores
 * them in caller-owned slots, pushes one frame, then reloads the slots after
 * every such runtime entry. Frames are thread-local and strictly LIFO;
 * push/pop themselves never allocate or trigger GC. */
typedef struct ScoopNativeRootFrame {
    struct ScoopNativeRootFrame *previous;
    void ***slots; /* array of addresses of managed-reference slots */
    uint64_t count;
} ScoopNativeRootFrame;

/* Addressable inline values held by managed runtime or Scoop-ABI native C
 * code use a distinct recursive-region root chain. The scan is relative to
 * `base`; unlike object TypeDescriptor scans, it contains no implicit object
 * header offset. */
typedef struct ScoopNativeRegionRootEntry {
    void *base;
    const uint64_t *scan;
} ScoopNativeRegionRootEntry;

typedef struct ScoopNativeRegionRootFrame {
    struct ScoopNativeRegionRootFrame *previous;
    ScoopNativeRegionRootEntry *entries;
    uint64_t count;
} ScoopNativeRegionRootFrame;

/* Compiler-published roots that stay live across one outbound native call.
 * Each entry scans one addressable value using the same recursive descriptor
 * format as object payloads. Inline tagged enums expose fixed ref offsets;
 * their inactive ref-bearing slots are zero. The base remains updateable for
 * moving GC. */
typedef struct ScoopCallerRootEntry {
    void *base;
    const uint64_t *scan;
} ScoopCallerRootEntry;

typedef struct ScoopCallerRootFrame {
    struct ScoopCallerRootFrame *previous;
    ScoopCallerRootEntry *entries;
    uint64_t count;
} ScoopCallerRootFrame;

/* Roots spilled around a managed invoke. This is deliberately a distinct
 * frame family from outbound-native caller roots: pushing it does not enter
 * a native transition, and unwinding pops the dynamic top frame after the
 * landingpad has captured the exception record. */
typedef struct ScoopCompilerRootFrame {
    struct ScoopCompilerRootFrame *previous;
    ScoopCallerRootEntry *entries;
    uint64_t count;
} ScoopCompilerRootFrame;

/* Stack-owned outbound transition record. Its fields are runtime-managed;
 * generated code allocates the record, passes it to enter/leave, and must not
 * copy or inspect it while active. The caller-root frame covers the generated
 * transition caller itself; return_pc/stack_pointer/frame_pointer anchor exact
 * stack-map traversal of the remaining frozen managed segment. */
typedef struct ScoopThreadTransition {
    struct ScoopThreadTransition *previous;
    ScoopCallerRootFrame *caller_roots;
    uintptr_t managed_return_pc;
    uintptr_t managed_stack_pointer;
    uintptr_t managed_frame_pointer;
    uintptr_t managed_stack_low;
    uintptr_t managed_stack_high;
    uint32_t previous_mode;
    uint32_t native_mode;
} ScoopThreadTransition;

void scoop_rt_push_native_roots(ScoopNativeRootFrame *frame, void ***slots,
                                uint64_t count);
void scoop_rt_pop_native_roots(ScoopNativeRootFrame *frame);
void scoop_rt_push_native_region_roots(ScoopNativeRegionRootFrame *frame,
                                       ScoopNativeRegionRootEntry *entries,
                                       uint64_t count);
void scoop_rt_pop_native_region_roots(ScoopNativeRegionRootFrame *frame);

void scoop_rt_push_caller_roots(ScoopCallerRootFrame *frame,
                                ScoopCallerRootEntry *entries,
                                uint64_t count);
void scoop_rt_pop_caller_roots(ScoopCallerRootFrame *frame);
void scoop_rt_push_compiler_roots(ScoopCompilerRootFrame *frame,
                                  ScoopCallerRootEntry *entries,
                                  uint64_t count);
void scoop_rt_pop_compiler_roots(ScoopCompilerRootFrame *frame);
void scoop_rt_pop_top_compiler_roots(void);
void scoop_rt_enter_native_safe(ScoopThreadTransition *transition,
                                uintptr_t managed_stack_low);
void scoop_rt_leave_native_safe(ScoopThreadTransition *transition);
void scoop_rt_enter_native_borrowed(ScoopThreadTransition *transition,
                                    uintptr_t managed_stack_low);
void scoop_rt_leave_native_borrowed(ScoopThreadTransition *transition);

/* pin / unpin (runtime spec 3.4): O(1) object-header flag mirrored in side
 * metadata, with no handle-table indirection. Returns the stable object
 * address. Pinning is not ref-counted: one unpin clears any number of pins. */
const void *scoop_rt_pin(const void *obj);
const void *scoop_rt_unpin(const void *obj);

/* GcHandle table (runtime spec 3.4): keeps the object alive without
 * pinning. A nonzero 64-bit value encodes generation and slot+1, so stale
 * handles cannot alias a reused slot; 0 remains the null niche.
 * release/resolve validate both components and abort for stale handles.
 * M15 updates live entries in place during relocation. */
uint64_t scoop_rt_get_handle(const void *obj);
const void *scoop_rt_release_handle(uint64_t handle);
const void *scoop_rt_resolve_handle(uint64_t handle);

/* M13 managed foreign-callback gateway (runtime spec 8). A generated typed
 * adapter receives the current closure object plus C argument/result storage,
 * catches every Scoop exception, and reports it through exception_out. */
typedef uint64_t (*ScoopForeignCallbackAdapter)(
    const void *closure, void *result_storage,
    const void *const *argument_storage, void **exception_out);

enum {
    SCOOP_FOREIGN_CALLBACK_REUSABLE = 0,
    SCOOP_FOREIGN_CALLBACK_ONE_SHOT = 1,
};

enum {
    SCOOP_FOREIGN_CALLBACK_REGISTERED = 0,
    SCOOP_FOREIGN_CALLBACK_ACTIVE = 1,
    SCOOP_FOREIGN_CALLBACK_COMPLETED = 2,
    SCOOP_FOREIGN_CALLBACK_FAILED = 3,
};

enum {
    SCOOP_FOREIGN_CALLBACK_RETURNED = 0,
    SCOOP_FOREIGN_CALLBACK_THREW = 1,
};

void scoop_callback_runtime_init(void);
void scoop_callback_prepare_shutdown(void);
void *scoop_runtime_callback_register(const void *closure,
                                      ScoopForeignCallbackAdapter adapter,
                                      const void *signature_descriptor,
                                      uint32_t mode);
void *scoop_runtime_callback_retain(void *context);
void scoop_runtime_callback_release(void *context);
uint32_t scoop_runtime_callback_state(void *context);
const void *scoop_runtime_callback_failure(void *context);
uint32_t scoop_runtime_callback_invoke(
    void *context, const void *signature_descriptor, void *result_storage,
    const void *const *argument_storage);
uint64_t scoop_runtime_callback_debug_live_count(void);
uint64_t scoop_runtime_callback_debug_owner_count(void *context);
uint64_t scoop_runtime_callback_debug_active_count(void *context);

/* ManagedEntry used by generated Scoop code to force a full collection. */
void scoop_rt_gc_collect(void);

/* NativeBorrowedEntry used by Scoop-ABI native implementations after their
 * caller/native root frames have been published. It never captures a C frame
 * as a managed anchor. */
void scoop_runtime_gc_collect(void);

/* Number of objects currently allocated from the GC heap. Includes
 * not-yet-collected garbage; right after scoop_rt_gc_collect() it is
 * the precise live count. Test/diagnostic hook (milestone9 DESIGN 1:
 * gcStats). */
uint64_t scoop_rt_gc_stats(void);

/* Test hook: number of heap blocks currently live in the arena. */
uint64_t scoop_rt_gc_debug_block_count(void);
uint64_t scoop_rt_gc_debug_last_moved_count(void);
uint64_t scoop_rt_gc_debug_allocation_size(const void *obj);

/* Test hook: base address of the heap arena the blocks are carved
 * from (0 before the first allocation / gc init). */
uintptr_t scoop_rt_gc_debug_arena_base(void);

/* Test hook: number of registered global/external roots. */
uint64_t scoop_rt_gc_debug_root_count(void);

/* Test hook: whether a pointer is the start of a currently allocated object. */
bool scoop_rt_gc_debug_is_allocated(const void *obj);

/* Test hook: number of slots in the current thread's native-root chain. */
uint64_t scoop_rt_gc_debug_native_root_count(void);

/* M13 thread-registry test hooks. Stack bounds are the current attached
 * pthread's inclusive-low/exclusive-high reserved stack range. */
enum {
    SCOOP_THREAD_DEBUG_NATIVE_SAFE = 0,
    SCOOP_THREAD_DEBUG_MANAGED = 1,
    SCOOP_THREAD_DEBUG_NATIVE_BORROWED = 2,
    SCOOP_THREAD_DEBUG_PARKED = 3,
    SCOOP_THREAD_DEBUG_COLLECTOR = 4,
    SCOOP_THREAD_DEBUG_DETACHING = 5,
};

bool scoop_rt_thread_debug_is_attached(void);
uint32_t scoop_rt_thread_debug_mode(void);
uint64_t scoop_rt_thread_debug_count(void);
uintptr_t scoop_rt_thread_debug_stack_low(void);
uintptr_t scoop_rt_thread_debug_stack_high(void);
void scoop_rt_thread_debug_enter_managed(uintptr_t managed_stack_boundary);
void scoop_rt_thread_debug_leave_managed(void);
uint64_t scoop_rt_thread_debug_gc_epoch(void);
uint64_t scoop_rt_thread_debug_last_gc_parked_count(void);
uint64_t scoop_rt_thread_debug_last_gc_native_safe_count(void);
uint64_t scoop_rt_thread_debug_caller_root_count(void);
uint64_t scoop_rt_thread_debug_compiler_root_count(void);
uint64_t scoop_rt_thread_debug_transition_depth(void);

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

/* Copy an ABI exception buffer back into an ordinary GC-managed object.
 * The freshly allocated object's header is retained; only the payload
 * after ScoopObjectHeader is copied from `caught` (runtime spec 5). */
void *scoop_rt_materialize_exception(const void *caught);

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
