/* Scoop runtime: object model and native FFI entry points.
 *
 * See docs/specs/SCOOP-RUNTIME-SPEC.md section 2 (object model) and
 * docs/milestone2/DESIGN.md section 3 (M2 runtime additions). M9
 * (docs/milestone9/DESIGN.md section 2) replaces the always-leak
 * allocator with the Immix-core GC in runtime/src/gc.c and widens the
 * object header to 16 bytes.
 */
#ifndef SCOOP_RT_H
#define SCOOP_RT_H

#include "scoop_runtime_metadata_v1.h"
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
#define SCOOP_RT_NORETURN [[noreturn]]
extern "C" {
#else
#define SCOOP_RT_NORETURN _Noreturn
#endif

/* GC scan-program contract (runtime spec 2.2).
 *
 * The GC finds an object's outgoing references purely through
 * `object_scan`; `type_id` is not interpreted by the GC.
 *
 * `object_scan` points to a sequence of u64 words whose first word
 * selects the kind:
 *
 * - NULL: the object has no outgoing references (String, plain objects
 *   without reference fields, arrays of non-reference elements).
 * - SCOOP_REFS_ARRAY: array object. Words 1..3 are the count offset,
 *   first-element offset, and nonzero element stride. Word 4 is a pointer
 *   (stored as u64) to the nonempty recursive scan program for one element.
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

/* Runtime spec 2.1: 16 bytes. `gc_word` belongs to the GC and currently
 * carries the pin and release-ready flags plus the explicit pin registry index.
 * Mark, exact size and forwarding state live in the arena-external side table.
 * Only compiler-generated constructor publication may set release-ready.
 * Source code must not touch it. TypeDescriptor shape determines each
 * aligned payload offset. */
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
 * header + element count + padding to the element alignment + inline
 * elements (stride = element layout size, known to codegen; elements
 * of value types are unboxed). `elements` names the minimum-alignment
 * offset; codegen computes the actual aligned data offset. */
typedef struct ScoopArray {
  ScoopObjectHeader header;
  uint64_t size;
  char elements[];
} ScoopArray;

/* NativeBorrowedEntry for Scoop-ABI C implementations. Generated code uses
 * the inline allocation context and falls back to the distinct managed entry
 * above. The native caller must already have published caller/native roots. */
void *scoop_rt_alloc(const ScoopTypeDescriptor *td, size_t size);

/* After writing references into managed heap storage, mark the complete
 * destination range before parking or publishing it. This is a GC leaf:
 * no allocation, lock or handshake. Empty ranges never access destination. */
void scoop_rt_gc_write_barrier(const void *destination, size_t bytes);

void scoop_rt_write(const ScoopString *s);
void scoop_rt_println(const ScoopString *s);
void scoop_rt_stdout_write(const uint8_t *bytes, int64_t length);
void scoop_rt_stderr_write(const uint8_t *bytes, int64_t length);
void scoop_rt_flush_stdout(void);

/* Fixed-width primitive conversions used by ordinary core methods. */
const ScoopString *scoop_rt_long_to_string(int64_t v);
const ScoopString *scoop_rt_ulong_to_string(uint64_t v);
const ScoopString *scoop_rt_bool_to_string(bool v);
const ScoopString *scoop_rt_char_to_string(uint32_t value);
const ScoopString *scoop_rt_float_to_string(float value);
const ScoopString *scoop_rt_double_to_string(double value);
int64_t scoop_rt_string_byte_length(const ScoopString *value);
int64_t scoop_rt_string_length(const ScoopString *value);
uint32_t scoop_rt_string_character_at_byte(const ScoopString *value, int64_t index);
int8_t scoop_rt_string_byte_at(const ScoopString *value, int64_t index);
const ScoopString *scoop_rt_string_slice_bytes(const ScoopString *value, int64_t start,
                                               int64_t end);
const ScoopString *scoop_rt_string_from_chars(const ScoopArray *value);
const ScoopString *scoop_rt_string_from_bytes(const ScoopArray *value);
const ScoopString *scoop_rt_string_join_parts(const ScoopArray *storage, int64_t part_count);
bool scoop_rt_bool_equals(bool left, bool right);
int64_t scoop_rt_long_hash(int64_t v);
int64_t scoop_rt_ulong_hash(uint64_t v);
int64_t scoop_rt_bool_hash(bool v);
int64_t scoop_rt_string_hash(const ScoopString *s);

/* String and primitive operations used by ordinary scoop.core declarations. */
bool scoop_rt_string_eq(const ScoopString *a, const ScoopString *b);
int64_t scoop_rt_string_compare(const ScoopString *a, const ScoopString *b);
void scoop_rt_print_long(int64_t value);
void scoop_rt_println_long(int64_t value);
void scoop_rt_print_boolean(bool value);
void scoop_rt_println_boolean(bool value);

/* Language termination flushes stdio in NativeSafe before ending the process.
 * Nothing retains its reference carrier; this function never returns it. */
SCOOP_RT_NORETURN void *scoop_rt_exit(int32_t code);
SCOOP_RT_NORETURN void scoop_rt_trap(const char *message);

/* Checked core collection growth cannot represent another Long element. */
SCOOP_RT_NORETURN void scoop_rt_allocation_overflow(void);

/* M6 additions (milestone6 DESIGN section 3): dispatch support. */

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

/* M13 thread registration baseline (runtime spec 3.5 / 7). A foreign
 * thread must attach before any later managed callback entry. The return
 * value is true only when this call created the attachment; nested users
 * must detach only when they own that true result. A newly attached foreign
 * thread starts in native-safe mode and cannot execute managed code until a
 * callback gateway performs the managed transition. */
bool scoop_rt_attach_foreign_thread(void);
void scoop_rt_detach_foreign_thread(void);

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

void scoop_rt_push_native_roots(ScoopNativeRootFrame *frame, void ***slots, uint64_t count);
void scoop_rt_pop_native_roots(ScoopNativeRootFrame *frame);
void scoop_rt_push_native_region_roots(ScoopNativeRegionRootFrame *frame,
                                       ScoopNativeRegionRootEntry *entries, uint64_t count);
void scoop_rt_pop_native_region_roots(ScoopNativeRegionRootFrame *frame);

/* pin / unpin (runtime spec 3.4): amortized O(1) pin and O(1) unpin, with
 * one counted registration per object. Each pin needs a matching unpin.
 * Both return the object address; copying a pointer adds no ownership. */
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

/* Managed foreign-callback gateway (runtime spec 4.3 and 9.4). The generated
 * adapter receives the closure and registration snapshot plus C storage,
 * catches Scoop exceptions, and reports them through exception_out. */
typedef uint64_t (*ScoopForeignCallbackAdapter)(const void *closure, const void *snapshot,
                                                void *result_storage,
                                                const void *const *argument_storage,
                                                void **exception_out);

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
void *scoop_runtime_callback_register(const void *closure, ScoopForeignCallbackAdapter adapter,
                                      const void *signature_descriptor, uint32_t mode);
void *scoop_runtime_callback_retain(void *context);
void scoop_runtime_callback_release(void *context);
uint32_t scoop_runtime_callback_state(void *context);
const void *scoop_runtime_callback_failure(void *context);
uint32_t scoop_runtime_callback_invoke(void *context, const void *signature_descriptor,
                                       void *result_storage, const void *const *argument_storage);
uint64_t scoop_runtime_callback_debug_live_count(void);
uint64_t scoop_runtime_callback_debug_owner_count(void *context);
uint64_t scoop_runtime_callback_debug_active_count(void *context);

/* NativeBorrowedEntry used by Scoop-ABI native implementations after their
 * caller/native root frames have been published. It never captures a C frame
 * as a managed anchor. */
void scoop_runtime_gc_collect(void);

/* Number of objects currently allocated from the GC heap. Includes
 * not-yet-collected garbage; right after scoop_runtime_gc_collect() it is
 * the precise live count. Test/diagnostic hook (milestone9 DESIGN 1:
 * gcStats). */
uint64_t scoop_rt_gc_stats(void);

typedef struct ScoopGcMetrics {
  uint64_t minor_collections;
  uint64_t full_collections;
  uint64_t promotion_fallbacks;
  uint64_t allocated_bytes;
  uint64_t nursery_allocated_bytes;
  uint64_t promoted_bytes;
  uint64_t dirty_cards;
  uint64_t old_reference_slots;
  uint64_t root_slots;
  uint64_t traced_objects;
  uint64_t pause_ns;
  uint64_t maximum_pause_ns;
  uint64_t heap_committed_bytes;
  uint64_t region_count;
  uint64_t large_mapping_count;
  uint64_t mapped_bytes;
  uint64_t discard_calls;
  uint64_t discard_failures;
  uint64_t discarded_bytes;
  uint64_t unmapped_bytes;
  uint64_t current_rss_bytes;
  uint64_t peak_rss_bytes;
} ScoopGcMetrics;

/* Diagnostic snapshot; counters never control program validity. */
void scoop_rt_gc_debug_metrics(ScoopGcMetrics *result);

/* Test hook: number of active ordinary blocks and large mappings. */
uint64_t scoop_rt_gc_debug_block_count(void);
uint64_t scoop_rt_gc_debug_last_moved_count(void);
uint64_t scoop_rt_gc_debug_allocation_size(const void *obj);

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

/* Read-only raw process arguments, available during eager initialization. */
int32_t scoop_rt_program_argc(void);
const char *scoop_rt_program_argv(int32_t index);

#undef SCOOP_RT_NORETURN

#ifdef __cplusplus
}
#endif

#endif /* SCOOP_RT_H */
