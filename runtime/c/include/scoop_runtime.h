// Stable public Scoop runtime core surface for cone-local native sources.
//
// This header intentionally exposes only runtime substrate that native sources
// may rely on across cone boundaries. Runtime implementation details such as
// heap backends, thread lists, platform shims, and private root machinery remain
// private to runtime/c.

#ifndef SCOOP_RUNTIME_H
#define SCOOP_RUNTIME_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define SCOOP_RUNTIME_ABI_VERSION 0u

typedef void (*ScoopGcTraceVisitor)(void **slot, void *ctx);
typedef uint64_t (*ScoopTypeTraceFn)(void *object,
                                     ScoopGcTraceVisitor visitor,
                                     void *ctx);
typedef void (*ScoopTypeReleaseFn)(void *object);

typedef struct ScoopTypeDescriptor {
  uint32_t abi_version;
  uint32_t flags;
  uint64_t size_bytes;
  uint64_t align_bytes;
  uint64_t trace_start_offset_bytes;
  uint32_t trace_bitmap_u64_len;
  uint32_t _reserved_u32;
  const uint64_t *trace_bitmap;
  ScoopTypeTraceFn trace_fn;
  ScoopTypeReleaseFn release_fn;
  uint64_t type_id;
  const struct ScoopTypeDescriptor *parent_type_desc;
  const void *itable;
  const void *vtable;
} ScoopTypeDescriptor;

typedef struct ScoopCompositeTransportDescriptor ScoopCompositeTransportDescriptor;

typedef uint64_t (*ScoopCompositeTraceFn)(
    const ScoopCompositeTransportDescriptor *descriptor,
    void *value,
    ScoopGcTraceVisitor visitor,
    void *ctx);
typedef void (*ScoopCompositeCopyFn)(
    const ScoopCompositeTransportDescriptor *descriptor,
    void *dst,
    const void *src);
typedef void (*ScoopCompositeDropFn)(
    const ScoopCompositeTransportDescriptor *descriptor,
    void *value);

struct ScoopCompositeTransportDescriptor {
  uint32_t abi_version;
  uint32_t storage_kind;
  uint64_t size_bytes;
  uint64_t align_bytes;
  const uint64_t *gc_slot_offsets;
  uint32_t gc_slot_count;
  uint32_t _reserved_u32;
  ScoopCompositeTraceFn trace_fn;
  ScoopCompositeCopyFn copy_fn;
  ScoopCompositeDropFn drop_fn;
  const ScoopTypeDescriptor *type_desc;
};

typedef struct ScoopString ScoopString;
typedef struct ScoopArray ScoopArray;
typedef struct ScoopMutableArray ScoopMutableArray;

// Opaque prefix for GC-managed objects allocated by cone-local native code.
// Native code may embed this as the first field when calling `scoop_alloc_typed`,
// but must not inspect or mutate the reserved words.
typedef struct ScoopObjectHeader {
  uintptr_t _scoop_runtime_private_next;
  uintptr_t _scoop_runtime_private_type_desc;
  uint64_t _scoop_runtime_private_size_bytes;
  uint32_t _scoop_runtime_private_flags;
  uint32_t _scoop_runtime_private_mark;
} ScoopObjectHeader;

#define SCOOP_ARRAY_ELEM_KIND_UNKNOWN 0u
#define SCOOP_ARRAY_ELEM_KIND_WORD 1u
#define SCOOP_ARRAY_ELEM_KIND_REF 2u
#define SCOOP_ARRAY_ELEM_KIND_COMPOSITE 3u

extern const ScoopTypeDescriptor __scoop_type_desc_runtime__ScoopString;

void scoop_runtime_init(void);
uint32_t scoop_runtime_is_initialized(void);
uint32_t scoop_runtime_init_count(void);

void scoop_gc_thread_attach_current(void);
void scoop_gc_thread_detach_current(void);

void scoop_enter_native(void ***root_slots, uint32_t root_slots_len);
void scoop_leave_native(void);

void *scoop_alloc(uint64_t size_bytes);
void *scoop_alloc_typed(const ScoopTypeDescriptor *type_desc, uint64_t size_bytes);

uint32_t scoop_pin(void *obj);
uint32_t scoop_unpin(void *obj);
uint64_t scoop_handle_new(void *obj);
void *scoop_handle_get(uint64_t handle);
uint32_t scoop_handle_drop(uint64_t handle);

void *scoop_gc_write_barrier(void *slot_addr, void *value);
void scoop_gc_register_global_root(void *base, const ScoopTypeDescriptor *type_desc);

uint64_t scoop_composite_trace(const ScoopCompositeTransportDescriptor *descriptor,
                               void *value,
                               ScoopGcTraceVisitor visitor,
                               void *ctx);
void scoop_composite_copy(const ScoopCompositeTransportDescriptor *descriptor,
                          void *dst,
                          const void *src);
void scoop_composite_drop(const ScoopCompositeTransportDescriptor *descriptor, void *value);

void *scoop_mutable_array_new(uint32_t elem_kind,
                              uint64_t elem_size,
                              uint64_t elem_align,
                              const void *elem_desc,
                              uint64_t capacity);
void scoop_mutable_array_push_word(void *mutable_array, uint64_t value);
void scoop_mutable_array_push_ref(void *mutable_array, void *value);
void scoop_mutable_array_push_composite(void *mutable_array,
                                        const void *slot_ptr,
                                        uint64_t elem_size);
const void *scoop_mutable_array_to_array_data(void *mutable_array);
void *scoop_mutable_array_freeze(void *mutable_array);

void scoop_print(const ScoopString *value);
void scoop_println(const ScoopString *value);
void scoop_panic(const void *message);
void scoop_runtime_error_fatal(const void *runtime_error);
void *scoop_entry_argv_array(int32_t argc, const char **argv);

const ScoopString *scoop_string_concat(const ScoopString *a, const ScoopString *b);
int64_t scoop_string_equals(const ScoopString *a, const ScoopString *b);
const ScoopString *scoop_string_unsafe_slice_bytes(const ScoopString *source,
                                                   int64_t byte_offset,
                                                   int64_t byte_length);

#ifdef __cplusplus
}
#endif

#endif // SCOOP_RUNTIME_H
