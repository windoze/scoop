// Scoop C runtime: Array / MutableArray primitive support.
//
// Array elements can be carried as scalar words, GC refs, or descriptor-backed
// composite values.  Composite values use ScoopCompositeTransportDescriptor for
// element size/alignment and trace/copy/drop hooks; scalar/ref arrays keep the
// existing public ABI.

#include <stdint.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

#include "scoop_array_internal.h"
#include "scoop_gc.h"

// `scoop_alloc` 在 `scoop_runtime.c` 中实现；这里仅声明供本模块使用。
void *scoop_alloc(uint64_t size);
void *scoop_alloc_typed(const ScoopTypeDescriptor *type_desc, uint64_t size_bytes);

#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
_Static_assert(offsetof(ScoopArray, header) == 0,
               "ScoopArray.header offset must stay at 0");
_Static_assert(offsetof(ScoopArray, len) == sizeof(ScoopGcObjectHeader),
               "ScoopArray.len offset must follow the GC header");
_Static_assert(offsetof(ScoopArray, elem_size_bytes) ==
                   sizeof(ScoopGcObjectHeader) + sizeof(uint64_t),
               "ScoopArray.elem_size_bytes offset drifted");
_Static_assert(offsetof(ScoopArray, data_offset_bytes) ==
                   sizeof(ScoopGcObjectHeader) + sizeof(uint64_t) * 2u,
               "ScoopArray.data_offset_bytes offset drifted");
_Static_assert(offsetof(ScoopArray, elem_desc) ==
                   sizeof(ScoopGcObjectHeader) + sizeof(uint64_t) * 3u,
               "ScoopArray.elem_desc offset drifted");
_Static_assert(offsetof(ScoopArray, elem_kind) ==
                   sizeof(ScoopGcObjectHeader) + sizeof(uint64_t) * 3u + sizeof(void *),
               "ScoopArray.elem_kind offset drifted");
_Static_assert(offsetof(ScoopArray, data) >= offsetof(ScoopArray, elem_kind) + sizeof(uint32_t) * 2u,
                "ScoopArray.data must remain after the fixed header fields");
_Static_assert(offsetof(ScoopMutableArray, header) == 0,
               "ScoopMutableArray.header offset must stay at 0");
_Static_assert(offsetof(ScoopMutableArray, len) == sizeof(ScoopGcObjectHeader),
               "ScoopMutableArray.len offset must follow the GC header");
_Static_assert(offsetof(ScoopMutableArray, cap) ==
                   sizeof(ScoopGcObjectHeader) + sizeof(uint64_t),
               "ScoopMutableArray.cap offset drifted");
_Static_assert(offsetof(ScoopMutableArray, elem_size_bytes) ==
                   sizeof(ScoopGcObjectHeader) + sizeof(uint64_t) * 2u,
               "ScoopMutableArray.elem_size_bytes offset drifted");
_Static_assert(offsetof(ScoopMutableArray, elem_align_bytes) ==
                   sizeof(ScoopGcObjectHeader) + sizeof(uint64_t) * 3u,
               "ScoopMutableArray.elem_align_bytes offset drifted");
_Static_assert(offsetof(ScoopMutableArray, elem_desc) ==
                   sizeof(ScoopGcObjectHeader) + sizeof(uint64_t) * 4u,
               "ScoopMutableArray.elem_desc offset drifted");
_Static_assert(offsetof(ScoopMutableArray, data) ==
                   sizeof(ScoopGcObjectHeader) + sizeof(uint64_t) * 4u + sizeof(void *),
               "ScoopMutableArray.data offset drifted");
_Static_assert(offsetof(ScoopMutableArray, elem_kind) ==
                   sizeof(ScoopGcObjectHeader) + sizeof(uint64_t) * 4u + sizeof(void *) * 2u,
               "ScoopMutableArray.elem_kind offset drifted");
#endif

static uint64_t scoop_array_align_up_u64(uint64_t value, uint64_t align) {
  if (align <= 1) {
    return value;
  }
  uint64_t rem = value % align;
  if (rem == 0) {
    return value;
  }
  uint64_t add = align - rem;
  if (UINT64_MAX - value < add) {
    return 0;
  }
  return value + add;
}

static uint64_t scoop_array_word_size(void) {
  return (uint64_t)sizeof(uintptr_t);
}

static uint64_t scoop_array_normalize_word_size(uint64_t elem_size) {
  return elem_size == 0 ? 1u : elem_size;
}

static uint64_t scoop_array_normalize_word_align(uint64_t elem_align) {
  return elem_align == 0 ? 1u : elem_align;
}

static uint32_t scoop_array_is_power_of_two(uint64_t value) {
  return value != 0 && (value & (value - 1u)) == 0;
}

static uint64_t scoop_array_element_size(uint32_t kind,
                                         uint64_t elem_size,
                                         const ScoopCompositeTransportDescriptor *desc) {
  if (kind == SCOOP_ARRAY_ELEM_KIND_COMPOSITE) {
    if (desc == 0 || desc->size_bytes == 0) {
      return 0;
    }
    return desc->size_bytes;
  }
  if (kind == SCOOP_ARRAY_ELEM_KIND_WORD) {
    return scoop_array_normalize_word_size(elem_size);
  }
  if (kind == SCOOP_ARRAY_ELEM_KIND_REF) {
    return scoop_array_word_size();
  }
  return 0;
}

static uint64_t scoop_array_element_align(uint32_t kind,
                                          uint64_t elem_align,
                                          const ScoopCompositeTransportDescriptor *desc) {
  if (kind == SCOOP_ARRAY_ELEM_KIND_COMPOSITE) {
    if (desc == 0 || desc->align_bytes == 0) {
      return 0;
    }
    return desc->align_bytes;
  }
  if (kind == SCOOP_ARRAY_ELEM_KIND_WORD) {
    return scoop_array_normalize_word_align(elem_align);
  }
  if (kind == SCOOP_ARRAY_ELEM_KIND_REF) {
    return (uint64_t)_Alignof(uintptr_t);
  }
  return 0;
}

static uint8_t *scoop_array_data(ScoopArray *arr) {
  if (arr == 0 || arr->data_offset_bytes == 0) {
    return 0;
  }
  return ((uint8_t *)arr) + arr->data_offset_bytes;
}

static uint64_t scoop_array_max_len_from_size(const ScoopArray *arr) {
  if (arr == 0 || arr->elem_size_bytes == 0 || arr->data_offset_bytes == 0) {
    return 0;
  }
  uint64_t size_bytes = arr->header.size_bytes;
  if (size_bytes <= arr->data_offset_bytes) {
    return 0;
  }
  return (size_bytes - arr->data_offset_bytes) / arr->elem_size_bytes;
}

static uint64_t scoop_array_trace_elems(void *object, ScoopGcTraceVisitor visitor, void *ctx) {
  if (object == 0 || visitor == 0) {
    return 0;
  }

  ScoopArray *arr = (ScoopArray *)object;
  uint8_t *data = scoop_array_data(arr);
  if (data == 0) {
    return 0;
  }

  uint64_t len = arr->len;
  uint64_t max_len = scoop_array_max_len_from_size(arr);
  if (len > max_len) {
    len = max_len;
  }

  uint64_t visited = 0;
  if (arr->elem_kind == SCOOP_ARRAY_ELEM_KIND_REF) {
    for (uint64_t i = 0; i < len; i++) {
      void **slot = (void **)(data + (i * arr->elem_size_bytes));
      visitor(slot, ctx);
      visited += 1;
    }
    return visited;
  }

  if (arr->elem_kind == SCOOP_ARRAY_ELEM_KIND_COMPOSITE && arr->elem_desc != 0) {
    for (uint64_t i = 0; i < len; i++) {
      visited += scoop_composite_trace(
          arr->elem_desc,
          data + (i * arr->elem_size_bytes),
          visitor,
          ctx);
    }
  }
  return visited;
}

static uint64_t scoop_out_of_line_array_trace_elems(
    uint64_t len,
    uint64_t cap,
    uint64_t elem_size_bytes,
    uint32_t elem_kind,
    const ScoopCompositeTransportDescriptor *elem_desc,
    uint8_t *data,
    ScoopGcTraceVisitor visitor,
    void *ctx) {
  if (visitor == 0 || data == 0 || len == 0 || elem_size_bytes == 0) {
    return 0;
  }

  if (cap > 0 && len > cap) {
    len = cap;
  }

  uint64_t visited = 0;
  if (elem_kind == SCOOP_ARRAY_ELEM_KIND_REF) {
    for (uint64_t i = 0; i < len; i++) {
      void **slot = (void **)(data + (i * elem_size_bytes));
      visitor(slot, ctx);
      visited += 1;
    }
    return visited;
  }

  if (elem_kind == SCOOP_ARRAY_ELEM_KIND_COMPOSITE && elem_desc != 0) {
    for (uint64_t i = 0; i < len; i++) {
      visited += scoop_composite_trace(
          elem_desc,
          data + (i * elem_size_bytes),
          visitor,
          ctx);
    }
  }
  return visited;
}

static uint64_t scoop_mutable_array_trace_elems(void *object,
                                                ScoopGcTraceVisitor visitor,
                                                void *ctx) {
  if (object == 0) {
    return 0;
  }
  ScoopMutableArray *arr = (ScoopMutableArray *)object;
  return scoop_out_of_line_array_trace_elems(
      arr->len,
      arr->cap,
      arr->elem_size_bytes,
      arr->elem_kind,
      arr->elem_desc,
      arr->data,
      visitor,
      ctx);
}

static void scoop_out_of_line_array_drop_elements(
    uint64_t len,
    uint64_t cap,
    uint64_t elem_size_bytes,
    uint32_t elem_kind,
    const ScoopCompositeTransportDescriptor *elem_desc,
    uint8_t *data) {
  if (data == 0 || elem_kind != SCOOP_ARRAY_ELEM_KIND_COMPOSITE || elem_desc == 0 ||
      elem_size_bytes == 0) {
    return;
  }
  if (cap > 0 && len > cap) {
    len = cap;
  }
  for (uint64_t i = 0; i < len; i++) {
    scoop_composite_drop(elem_desc, data + (i * elem_size_bytes));
  }
}

static void scoop_mutable_array_release(void *object) {
  if (object == 0) {
    return;
  }

  ScoopMutableArray *arr = (ScoopMutableArray *)object;
  scoop_out_of_line_array_drop_elements(
      arr->len,
      arr->cap,
      arr->elem_size_bytes,
      arr->elem_kind,
      arr->elem_desc,
      arr->data);
  if (arr->data != 0) {
    free(arr->data);
    arr->data = 0;
  }
  arr->len = 0;
  arr->cap = 0;
  arr->elem_size_bytes = 0;
  arr->elem_align_bytes = 0;
  arr->elem_desc = 0;
  arr->elem_kind = SCOOP_ARRAY_ELEM_KIND_UNKNOWN;
}

typedef struct ScoopArrayCompositeSlotOffsets {
  uint64_t *items;
  uint32_t len;
  uint32_t cap;
  const uint8_t *base;
  uint64_t size_bytes;
  uint32_t failed;
} ScoopArrayCompositeSlotOffsets;

typedef struct ScoopArrayPreparedCompositeCopy {
  uint8_t *bytes;
  size_t size_bytes;
  ScoopArrayCompositeSlotOffsets slots;
} ScoopArrayPreparedCompositeCopy;

static uint32_t scoop_array_slot_offsets_push(ScoopArrayCompositeSlotOffsets *slots,
                                              uint64_t offset) {
  if (slots == 0) {
    return 0;
  }
  uint32_t insert_at = slots->len;
  for (uint32_t i = 0; i < slots->len; i++) {
    if (slots->items[i] == offset) {
      return 1;
    }
    if (slots->items[i] > offset) {
      insert_at = i;
      break;
    }
  }
  if (slots->len == slots->cap) {
    uint32_t new_cap = slots->cap == 0 ? 4u : slots->cap * 2u;
    if (new_cap < slots->cap) {
      return 0;
    }
    uint64_t *items = (uint64_t *)realloc(slots->items, sizeof(uint64_t) * new_cap);
    if (items == 0) {
      return 0;
    }
    slots->items = items;
    slots->cap = new_cap;
  }
  if (insert_at < slots->len) {
    (void)memmove(slots->items + insert_at + 1,
                  slots->items + insert_at,
                  sizeof(uint64_t) * (slots->len - insert_at));
  }
  slots->items[insert_at] = offset;
  slots->len += 1;
  return 1;
}

static void scoop_array_collect_composite_slot_offset(void **slot, void *ctx) {
  ScoopArrayCompositeSlotOffsets *slots = (ScoopArrayCompositeSlotOffsets *)ctx;
  if (slots == 0 || slot == 0 || slots->base == 0) {
    return;
  }
  uintptr_t base = (uintptr_t)slots->base;
  uintptr_t addr = (uintptr_t)slot;
  if (addr < base) {
    slots->failed = 1;
    return;
  }
  uint64_t offset = (uint64_t)(addr - base);
  if (offset > slots->size_bytes || slots->size_bytes - offset < sizeof(void *)) {
    slots->failed = 1;
    return;
  }
  if (!scoop_array_slot_offsets_push(slots, offset)) {
    slots->failed = 1;
  }
}

static uint32_t scoop_array_collect_composite_slot_offsets(
    const ScoopCompositeTransportDescriptor *descriptor,
    const void *value,
    ScoopArrayCompositeSlotOffsets *slots) {
  if (slots == 0) {
    return 0;
  }
  slots->items = 0;
  slots->len = 0;
  slots->cap = 0;
  slots->base = (const uint8_t *)value;
  slots->size_bytes = descriptor != 0 ? descriptor->size_bytes : 0;
  slots->failed = 0;
  if (descriptor == 0 || value == 0 || descriptor->size_bytes > (uint64_t)SIZE_MAX) {
    slots->failed = 1;
    return 0;
  }
  (void)scoop_composite_trace(
      descriptor, (void *)value, scoop_array_collect_composite_slot_offset, slots);
  return slots->failed == 0;
}

static void scoop_array_free_composite_slot_offsets(ScoopArrayCompositeSlotOffsets *slots) {
  if (slots == 0) {
    return;
  }
  free(slots->items);
  slots->items = 0;
  slots->len = 0;
  slots->cap = 0;
}

static uint32_t scoop_array_prepare_composite_copy(
    const ScoopCompositeTransportDescriptor *descriptor,
    const void *value,
    ScoopArrayPreparedCompositeCopy *prepared) {
  if (prepared == 0 || descriptor == 0 || value == 0 || descriptor->size_bytes == 0 ||
      descriptor->size_bytes > (uint64_t)SIZE_MAX) {
    return 0;
  }
  (void)memset(prepared, 0, sizeof(*prepared));
  prepared->size_bytes = (size_t)descriptor->size_bytes;
  prepared->bytes = (uint8_t *)malloc(prepared->size_bytes);
  if (prepared->bytes == 0) {
    return 0;
  }
  (void)memset(prepared->bytes, 0, prepared->size_bytes);
  scoop_composite_copy(descriptor, prepared->bytes, value);
  if (!scoop_array_collect_composite_slot_offsets(
          descriptor, prepared->bytes, &prepared->slots)) {
    scoop_array_free_composite_slot_offsets(&prepared->slots);
    free(prepared->bytes);
    prepared->bytes = 0;
    prepared->size_bytes = 0;
    return 0;
  }
  return 1;
}

static void scoop_array_destroy_prepared_composite_copy(
    ScoopArrayPreparedCompositeCopy *prepared) {
  if (prepared == 0) {
    return;
  }
  scoop_array_free_composite_slot_offsets(&prepared->slots);
  free(prepared->bytes);
  prepared->bytes = 0;
  prepared->size_bytes = 0;
}

static void scoop_array_copy_composite_non_gc_ranges(
    uint8_t *dst,
    const ScoopArrayPreparedCompositeCopy *prepared) {
  size_t cursor = 0;
  for (uint32_t i = 0; i < prepared->slots.len; i++) {
    size_t offset = (size_t)prepared->slots.items[i];
    if (offset > cursor) {
      (void)memcpy(dst + cursor, prepared->bytes + cursor, offset - cursor);
    }
    cursor = offset + sizeof(void *);
  }
  if (cursor < prepared->size_bytes) {
    (void)memcpy(dst + cursor, prepared->bytes + cursor, prepared->size_bytes - cursor);
  }
}

static void scoop_array_commit_composite_copy_to_gc_storage(
    uint8_t *dst,
    const ScoopArrayPreparedCompositeCopy *prepared) {
  if (prepared->slots.len == 0) {
    (void)memcpy(dst, prepared->bytes, prepared->size_bytes);
    return;
  }
  for (uint32_t i = 0; i < prepared->slots.len; i++) {
    uint64_t offset = prepared->slots.items[i];
    (void)memset(dst + offset, 0, sizeof(void *));
  }
  scoop_array_copy_composite_non_gc_ranges(dst, prepared);
  for (uint32_t i = 0; i < prepared->slots.len; i++) {
    uint64_t offset = prepared->slots.items[i];
    void *value = 0;
    (void)memcpy(&value, prepared->bytes + offset, sizeof(void *));
    (void)scoop_gc_write_barrier((void *)(dst + offset), value);
  }
}

static void scoop_array_release(void *object) {
  if (object == 0) {
    return;
  }
  ScoopArray *arr = (ScoopArray *)object;
  if (arr->elem_kind != SCOOP_ARRAY_ELEM_KIND_COMPOSITE || arr->elem_desc == 0 ||
      arr->elem_size_bytes == 0) {
    return;
  }
  uint8_t *data = scoop_array_data(arr);
  if (data == 0) {
    return;
  }
  uint64_t len = arr->len;
  uint64_t max_len = scoop_array_max_len_from_size(arr);
  if (len > max_len) {
    len = max_len;
  }
  for (uint64_t i = 0; i < len; i++) {
    scoop_composite_drop(arr->elem_desc, data + (i * arr->elem_size_bytes));
  }
  arr->len = 0;
}

static const ScoopTypeDescriptor SCOOP_ARRAY_TYPE_DESC = {
    .abi_version = 0,
    .flags = 0,
    .size_bytes = sizeof(ScoopArray),
    .align_bytes = (uint64_t)_Alignof(ScoopArray),
    .trace_start_offset_bytes = 0,
    .trace_bitmap_u64_len = 0,
    ._reserved_u32 = 0,
    .trace_bitmap = 0,
    .trace_fn = scoop_array_trace_elems,
    .release_fn = scoop_array_release,
    .type_id = 0,
    .parent_type_desc = 0,
    .itable = 0,
    .vtable = 0,
};

static const ScoopTypeDescriptor SCOOP_MUTABLE_ARRAY_TYPE_DESC = {
    .abi_version = 0,
    .flags = 0,
    .size_bytes = sizeof(ScoopMutableArray),
    .align_bytes = (uint64_t)_Alignof(ScoopMutableArray),
    .trace_start_offset_bytes = 0,
    .trace_bitmap_u64_len = 0,
    ._reserved_u32 = 0,
    .trace_bitmap = 0,
    .trace_fn = scoop_mutable_array_trace_elems,
    .release_fn = scoop_mutable_array_release,
    .type_id = 0,
    .parent_type_desc = 0,
    .itable = 0,
    .vtable = 0,
};

static uint64_t scoop_array_allocation_size(
    uint64_t len,
    uint32_t elem_kind,
    uint64_t elem_size_hint,
    uint64_t elem_align_hint,
    const ScoopCompositeTransportDescriptor *desc,
    uint64_t *out_elem_size,
    uint64_t *out_data_offset) {
  uint64_t elem_size = scoop_array_element_size(elem_kind, elem_size_hint, desc);
  uint64_t elem_align = scoop_array_element_align(elem_kind, elem_align_hint, desc);
  if (elem_size == 0 || elem_align == 0) {
    return 0;
  }
  uint64_t data_offset = scoop_array_align_up_u64((uint64_t)sizeof(ScoopArray), elem_align);
  if (data_offset == 0) {
    return 0;
  }
  if (len > (uint64_t)(UINT64_MAX / elem_size)) {
    return 0;
  }
  uint64_t data_bytes = len * elem_size;
  if (UINT64_MAX - data_offset < data_bytes) {
    return 0;
  }
  *out_elem_size = elem_size;
  *out_data_offset = data_offset;
  return data_offset + data_bytes;
}

static uint32_t scoop_mutable_array_layout(
    uint32_t elem_kind,
    uint64_t elem_size,
    uint64_t elem_align,
    const ScoopCompositeTransportDescriptor *descriptor,
    uint64_t *out_elem_size,
    uint64_t *out_elem_align,
    const ScoopCompositeTransportDescriptor **out_descriptor) {
  if (out_elem_size == 0 || out_elem_align == 0 || out_descriptor == 0) {
    return 0;
  }

  if (elem_kind == SCOOP_ARRAY_ELEM_KIND_WORD) {
    uint64_t normalized_size = scoop_array_normalize_word_size(elem_size);
    uint64_t normalized_align = scoop_array_normalize_word_align(elem_align);
    if (normalized_size > (uint64_t)sizeof(uint64_t) ||
        !scoop_array_is_power_of_two(normalized_align) ||
        normalized_align > normalized_size) {
      return 0;
    }
    *out_elem_size = normalized_size;
    *out_elem_align = normalized_align;
    *out_descriptor = 0;
    return 1;
  }

  if (elem_kind == SCOOP_ARRAY_ELEM_KIND_REF) {
    *out_elem_size = scoop_array_word_size();
    *out_elem_align = (uint64_t)_Alignof(uintptr_t);
    *out_descriptor = 0;
    return 1;
  }

  if (elem_kind != SCOOP_ARRAY_ELEM_KIND_COMPOSITE || descriptor == 0) {
    return 0;
  }

  uint64_t desc_size = descriptor->size_bytes;
  uint64_t desc_align = descriptor->align_bytes;
  if (desc_size == 0 || desc_align == 0) {
    return 0;
  }
  if ((elem_size != 0 && elem_size != desc_size) ||
      (elem_align != 0 && elem_align != desc_align)) {
    return 0;
  }

  *out_elem_size = desc_size;
  *out_elem_align = desc_align;
  *out_descriptor = descriptor;
  return 1;
}

static uint32_t scoop_mutable_array_alloc_data(
    uint64_t cap,
    uint64_t elem_size,
    uint8_t **out_data) {
  if (out_data == 0 || cap == 0 || elem_size == 0 || cap > (uint64_t)(SIZE_MAX / elem_size)) {
    return 0;
  }
  size_t bytes = (size_t)cap * (size_t)elem_size;
  uint8_t *data = (uint8_t *)malloc(bytes);
  if (data == 0) {
    return 0;
  }
  *out_data = data;
  return 1;
}

static uint32_t scoop_mutable_array_grow(ScoopMutableArray *arr) {
  if (arr == 0 || arr->elem_size_bytes == 0) {
    return 0;
  }

  uint64_t old_cap = arr->cap;
  uint64_t new_cap = old_cap == 0 ? 4u : old_cap * 2u;
  if (new_cap < old_cap || new_cap > (uint64_t)(SIZE_MAX / arr->elem_size_bytes)) {
    return 0;
  }

  size_t bytes = (size_t)new_cap * (size_t)arr->elem_size_bytes;
  uint8_t *data = (uint8_t *)realloc(arr->data, bytes);
  if (data == 0) {
    return 0;
  }

  arr->data = data;
  arr->cap = new_cap;
  return 1;
}

static uint8_t *scoop_mutable_array_next_slot(ScoopMutableArray *arr, uint32_t elem_kind) {
  if (arr == 0 || arr->data == 0 || arr->elem_kind != elem_kind || arr->elem_size_bytes == 0) {
    return 0;
  }
  if (arr->len >= arr->cap && !scoop_mutable_array_grow(arr)) {
    return 0;
  }
  if (arr->len >= arr->cap) {
    return 0;
  }
  return arr->data + (arr->len * arr->elem_size_bytes);
}

static void scoop_mutable_array_store_word(uint8_t *slot, uint64_t value, uint64_t elem_size) {
  if (slot == 0) {
    return;
  }
  switch (elem_size) {
    case 1: {
      uint8_t narrowed = (uint8_t)value;
      (void)memcpy(slot, &narrowed, sizeof(narrowed));
      return;
    }
    case 2: {
      uint16_t narrowed = (uint16_t)value;
      (void)memcpy(slot, &narrowed, sizeof(narrowed));
      return;
    }
    case 4: {
      uint32_t narrowed = (uint32_t)value;
      (void)memcpy(slot, &narrowed, sizeof(narrowed));
      return;
    }
    case 8:
      (void)memcpy(slot, &value, sizeof(value));
      return;
    default:
      (void)memset(slot, 0, (size_t)elem_size);
      return;
  }
}

static void scoop_array_promote_c_heap_ref_slot(void **slot, void *ctx) {
  (void)ctx;
  if (slot == 0) {
    return;
  }
  // Null slot means "promote/poll for an out-of-line C-heap store"; no slot write occurs.
  (void)scoop_gc_write_barrier(0, *slot);
}

void *scoop_mutable_array_new(uint32_t elem_kind,
                              uint64_t elem_size,
                              uint64_t elem_align,
                              const void *elem_desc,
                              uint64_t capacity) {
  const ScoopCompositeTransportDescriptor *descriptor =
      (const ScoopCompositeTransportDescriptor *)elem_desc;
  uint64_t normalized_size = 0;
  uint64_t normalized_align = 0;
  const ScoopCompositeTransportDescriptor *normalized_desc = 0;
  if (!scoop_mutable_array_layout(
          elem_kind,
          elem_size,
          elem_align,
          descriptor,
          &normalized_size,
          &normalized_align,
          &normalized_desc)) {
    return 0;
  }

  uint64_t cap = capacity < 4u ? 4u : capacity;
  uint8_t *data = 0;
  if (!scoop_mutable_array_alloc_data(cap, normalized_size, &data)) {
    return 0;
  }

  ScoopMutableArray *arr = (ScoopMutableArray *)scoop_alloc_typed(
      &SCOOP_MUTABLE_ARRAY_TYPE_DESC,
      (uint64_t)sizeof(ScoopMutableArray));
  if (arr == 0) {
    free(data);
    return 0;
  }

  arr->len = 0;
  arr->cap = cap;
  arr->elem_size_bytes = normalized_size;
  arr->elem_align_bytes = normalized_align;
  arr->elem_desc = normalized_desc;
  arr->data = data;
  arr->elem_kind = elem_kind;
  arr->_reserved_u32 = 0;
  return (void *)arr;
}

uint64_t scoop_mutable_array_len(const void *mutable_array) {
  const ScoopMutableArray *arr = (const ScoopMutableArray *)mutable_array;
  return arr == 0 ? 0 : arr->len;
}

uint32_t scoop_mutable_array_elem_kind(const void *mutable_array) {
  const ScoopMutableArray *arr = (const ScoopMutableArray *)mutable_array;
  return arr == 0 ? SCOOP_ARRAY_ELEM_KIND_UNKNOWN : arr->elem_kind;
}

uint64_t scoop_mutable_array_elem_size(const void *mutable_array) {
  const ScoopMutableArray *arr = (const ScoopMutableArray *)mutable_array;
  return arr == 0 ? 0 : arr->elem_size_bytes;
}

void scoop_mutable_array_push_word(void *mutable_array, uint64_t value) {
  ScoopMutableArray *arr = (ScoopMutableArray *)mutable_array;
  uint8_t *slot = scoop_mutable_array_next_slot(arr, SCOOP_ARRAY_ELEM_KIND_WORD);
  if (slot == 0) {
    return;
  }
  scoop_mutable_array_store_word(slot, value, arr->elem_size_bytes);
  arr->len += 1;
}

void scoop_mutable_array_push_ref(void *mutable_array, void *value) {
  ScoopMutableArray *arr = (ScoopMutableArray *)mutable_array;
  uint8_t *slot = scoop_mutable_array_next_slot(arr, SCOOP_ARRAY_ELEM_KIND_REF);
  if (slot == 0) {
    return;
  }
  (void)memcpy(slot, &value, sizeof(value));
  arr->len += 1;
  (void)scoop_gc_write_barrier(0, value);
}

void scoop_mutable_array_push_composite(void *mutable_array,
                                        const void *slot_ptr,
                                        uint64_t elem_size) {
  ScoopMutableArray *arr = (ScoopMutableArray *)mutable_array;
  if (arr == 0 || arr->elem_desc == 0 ||
      (elem_size != 0 && elem_size != arr->elem_size_bytes)) {
    return;
  }
  uint8_t *slot = scoop_mutable_array_next_slot(arr, SCOOP_ARRAY_ELEM_KIND_COMPOSITE);
  if (slot == 0) {
    return;
  }

  if (slot_ptr == 0) {
    (void)memset(slot, 0, (size_t)arr->elem_size_bytes);
  } else {
    scoop_composite_copy(arr->elem_desc, slot, slot_ptr);
  }
  arr->len += 1;
  (void)scoop_composite_trace(
      arr->elem_desc,
      slot,
      scoop_array_promote_c_heap_ref_slot,
      0);
}

const void *scoop_mutable_array_to_array_data(const void *mutable_array) {
  const ScoopMutableArray *arr = (const ScoopMutableArray *)mutable_array;
  if (arr == 0) {
    return 0;
  }
  return (const void *)arr->data;
}

void *scoop_mutable_array_freeze(void *mutable_array) {
  ScoopMutableArray *src = (ScoopMutableArray *)mutable_array;
  if (src == 0 || src->elem_kind == SCOOP_ARRAY_ELEM_KIND_UNKNOWN ||
      src->elem_size_bytes == 0) {
    return 0;
  }

  uint64_t elem_size = 0;
  uint64_t data_offset = 0;
  uint64_t bytes = scoop_array_allocation_size(
      src->len,
      src->elem_kind,
      src->elem_size_bytes,
      src->elem_align_bytes,
      src->elem_desc,
      &elem_size,
      &data_offset);
  if (bytes == 0) {
    return 0;
  }

  scoop_pin((void *)src);
  ScoopArray *arr = (ScoopArray *)scoop_alloc_typed(&SCOOP_ARRAY_TYPE_DESC, bytes);
  if (arr == 0) {
    scoop_unpin((void *)src);
    return 0;
  }
  scoop_pin((void *)arr);

  arr->len = 0;
  arr->elem_size_bytes = elem_size;
  arr->data_offset_bytes = data_offset;
  arr->elem_desc = src->elem_kind == SCOOP_ARRAY_ELEM_KIND_COMPOSITE ? src->elem_desc : 0;
  arr->elem_kind = src->elem_kind;
  arr->_reserved_u32 = 0;

  uint8_t *dst = scoop_array_data(arr);
  if (src->len > 0 && src->data != 0 && dst != 0) {
    if (src->elem_kind == SCOOP_ARRAY_ELEM_KIND_COMPOSITE && src->elem_desc != 0) {
      for (uint64_t i = 0; i < src->len; i++) {
        ScoopArrayPreparedCompositeCopy prepared;
        if (!scoop_array_prepare_composite_copy(
                src->elem_desc, src->data + (i * elem_size), &prepared)) {
          scoop_array_release((void *)arr);
          scoop_unpin((void *)arr);
          scoop_unpin((void *)src);
          return 0;
        }
        arr->len += 1;
        scoop_array_commit_composite_copy_to_gc_storage(dst + (i * elem_size), &prepared);
        scoop_array_destroy_prepared_composite_copy(&prepared);
      }
    } else if (src->elem_kind == SCOOP_ARRAY_ELEM_KIND_REF) {
      for (uint64_t i = 0; i < src->len; i++) {
        void *value = 0;
        uint8_t *slot = dst + (i * elem_size);
        (void)memcpy(&value, src->data + (i * elem_size), sizeof(void *));
        (void)scoop_gc_write_barrier((void *)slot, value);
        arr->len += 1;
      }
    } else {
      (void)memcpy(dst, src->data, (size_t)src->len * (size_t)elem_size);
      arr->len = src->len;
    }
  }

  scoop_unpin((void *)arr);
  scoop_unpin((void *)src);
  return (void *)arr;
}
