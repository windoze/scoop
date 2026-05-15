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

#include "scoop_gc.h"

// `scoop_alloc` 在 `scoop_runtime.c` 中实现；这里仅声明供本模块使用。
void *scoop_alloc(uint64_t size);
void *scoop_alloc_typed(const ScoopTypeDescriptor *type_desc, uint64_t size_bytes);

#define SCOOP_ARRAY_ELEM_KIND_UNKNOWN 0u
#define SCOOP_ARRAY_ELEM_KIND_WORD 1u
#define SCOOP_ARRAY_ELEM_KIND_REF 2u
#define SCOOP_ARRAY_ELEM_KIND_COMPOSITE 3u

typedef struct ScoopArray {
  ScoopGcObjectHeader header;
  uint64_t len;
  uint64_t elem_size_bytes;
  uint64_t data_offset_bytes;
  const ScoopCompositeTransportDescriptor *elem_desc;
  uint32_t elem_kind;
  uint32_t _reserved_u32;
  uint8_t data[];
} ScoopArray;

typedef struct ScoopArrayBuilder {
  ScoopGcObjectHeader header;
  uint64_t len;
  uint64_t cap;
  uint64_t elem_size_bytes;
  uint64_t elem_align_bytes;
  const ScoopCompositeTransportDescriptor *elem_desc;
  uint8_t *data;
  uint32_t elem_kind;
  uint32_t _reserved_u32;
} ScoopArrayBuilder;

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

static uint64_t scoop_array_element_size(uint32_t kind,
                                         const ScoopCompositeTransportDescriptor *desc) {
  if (kind == SCOOP_ARRAY_ELEM_KIND_COMPOSITE) {
    if (desc == 0 || desc->size_bytes == 0) {
      return 0;
    }
    return desc->size_bytes;
  }
  return scoop_array_word_size();
}

static uint64_t scoop_array_element_align(uint32_t kind,
                                          const ScoopCompositeTransportDescriptor *desc) {
  if (kind == SCOOP_ARRAY_ELEM_KIND_COMPOSITE) {
    if (desc == 0 || desc->align_bytes == 0) {
      return 0;
    }
    return desc->align_bytes;
  }
  return (uint64_t)_Alignof(uintptr_t);
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

static uint64_t scoop_array_builder_trace_elems(void *object,
                                                ScoopGcTraceVisitor visitor,
                                                void *ctx) {
  if (object == 0 || visitor == 0) {
    return 0;
  }

  ScoopArrayBuilder *b = (ScoopArrayBuilder *)object;
  if (b->data == 0 || b->len == 0 || b->elem_size_bytes == 0) {
    return 0;
  }

  uint64_t len = b->len;
  if (b->cap > 0 && len > b->cap) {
    len = b->cap;
  }

  uint64_t visited = 0;
  if (b->elem_kind == SCOOP_ARRAY_ELEM_KIND_REF) {
    for (uint64_t i = 0; i < len; i++) {
      void **slot = (void **)(b->data + (i * b->elem_size_bytes));
      visitor(slot, ctx);
      visited += 1;
    }
    return visited;
  }

  if (b->elem_kind == SCOOP_ARRAY_ELEM_KIND_COMPOSITE && b->elem_desc != 0) {
    for (uint64_t i = 0; i < len; i++) {
      visited += scoop_composite_trace(
          b->elem_desc,
          b->data + (i * b->elem_size_bytes),
          visitor,
          ctx);
    }
  }
  return visited;
}

static void scoop_array_builder_drop_elements(ScoopArrayBuilder *b) {
  if (b == 0 || b->data == 0 || b->elem_kind != SCOOP_ARRAY_ELEM_KIND_COMPOSITE ||
      b->elem_desc == 0 || b->elem_size_bytes == 0) {
    return;
  }
  uint64_t len = b->len;
  if (b->cap > 0 && len > b->cap) {
    len = b->cap;
  }
  for (uint64_t i = 0; i < len; i++) {
    scoop_composite_drop(b->elem_desc, b->data + (i * b->elem_size_bytes));
  }
}

static void scoop_array_builder_release(void *object) {
  if (object == 0) {
    return;
  }

  ScoopArrayBuilder *b = (ScoopArrayBuilder *)object;
  scoop_array_builder_drop_elements(b);
  if (b->data != 0) {
    free(b->data);
    b->data = 0;
  }
  b->len = 0;
  b->cap = 0;
  b->elem_size_bytes = 0;
  b->elem_align_bytes = 0;
  b->elem_desc = 0;
  b->elem_kind = SCOOP_ARRAY_ELEM_KIND_UNKNOWN;
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

static const ScoopTypeDescriptor SCOOP_ARRAY_BUILDER_TYPE_DESC = {
    .abi_version = 0,
    .flags = 0,
    .size_bytes = sizeof(ScoopArrayBuilder),
    .align_bytes = (uint64_t)_Alignof(ScoopArrayBuilder),
    .trace_start_offset_bytes = 0,
    .trace_bitmap_u64_len = 0,
    ._reserved_u32 = 0,
    .trace_bitmap = 0,
    .trace_fn = scoop_array_builder_trace_elems,
    .release_fn = scoop_array_builder_release,
    .type_id = 0,
    .parent_type_desc = 0,
    .itable = 0,
    .vtable = 0,
};

static uint32_t scoop_array_builder_configure(
    ScoopArrayBuilder *b,
    uint32_t elem_kind,
    const ScoopCompositeTransportDescriptor *desc) {
  if (b == 0) {
    return 0;
  }
  uint64_t elem_size = scoop_array_element_size(elem_kind, desc);
  uint64_t elem_align = scoop_array_element_align(elem_kind, desc);
  if (elem_size == 0 || elem_align == 0) {
    return 0;
  }

  if (b->elem_kind == SCOOP_ARRAY_ELEM_KIND_UNKNOWN) {
    b->elem_kind = elem_kind;
    b->elem_size_bytes = elem_size;
    b->elem_align_bytes = elem_align;
    b->elem_desc = (elem_kind == SCOOP_ARRAY_ELEM_KIND_COMPOSITE) ? desc : 0;
    b->header.type_desc = &SCOOP_ARRAY_BUILDER_TYPE_DESC;
    return 1;
  }

  if (b->elem_kind != elem_kind || b->elem_size_bytes != elem_size ||
      b->elem_align_bytes != elem_align) {
    return 0;
  }
  if (elem_kind == SCOOP_ARRAY_ELEM_KIND_COMPOSITE && b->elem_desc != desc) {
    return 0;
  }
  return 1;
}

static uint32_t scoop_array_builder_grow_impl(ScoopArrayBuilder *b) {
  if (b == 0 || b->elem_size_bytes == 0) {
    return 0;
  }

  uint64_t old_cap = b->cap;
  uint64_t new_cap = (old_cap == 0) ? 4u : old_cap * 2u;
  if (new_cap < old_cap) {
    return 0;
  }
  if (b->elem_size_bytes > 0 && new_cap > (uint64_t)(SIZE_MAX / b->elem_size_bytes)) {
    return 0;
  }

  size_t bytes = (size_t)new_cap * (size_t)b->elem_size_bytes;
  uint8_t *p = (uint8_t *)realloc(b->data, bytes);
  if (p == 0) {
    return 0;
  }

  b->data = p;
  b->cap = new_cap;
  return 1;
}

uint32_t scoop_array_builder_grow(void *builder) {
  return scoop_array_builder_grow_impl((ScoopArrayBuilder *)builder);
}

void *scoop_array_builder_new(void) {
  ScoopArrayBuilder *b = (ScoopArrayBuilder *)scoop_alloc((uint64_t)sizeof(ScoopArrayBuilder));
  if (b == 0) {
    return 0;
  }

  b->header.type_desc = &SCOOP_ARRAY_BUILDER_TYPE_DESC;
  b->len = 0;
  b->cap = 0;
  b->elem_size_bytes = 0;
  b->elem_align_bytes = 0;
  b->elem_desc = 0;
  b->data = 0;
  b->elem_kind = SCOOP_ARRAY_ELEM_KIND_UNKNOWN;
  b->_reserved_u32 = 0;
  return (void *)b;
}

void scoop_array_builder_push_u64(void *builder, uint64_t value) {
  ScoopArrayBuilder *b = (ScoopArrayBuilder *)builder;
  if (!scoop_array_builder_configure(b, SCOOP_ARRAY_ELEM_KIND_WORD, 0)) {
    return;
  }

  if (b->len >= b->cap && !scoop_array_builder_grow_impl(b)) {
    return;
  }

  (void)memcpy(b->data + (b->len * b->elem_size_bytes), &value, sizeof(value));
  b->len += 1;
}

void scoop_array_builder_push_ref(void *builder, void *value) {
  ScoopArrayBuilder *b = (ScoopArrayBuilder *)builder;
  if (!scoop_array_builder_configure(b, SCOOP_ARRAY_ELEM_KIND_REF, 0)) {
    return;
  }

  if (b->len >= b->cap && !scoop_array_builder_grow_impl(b)) {
    return;
  }

  uintptr_t encoded = (uintptr_t)value;
  (void)memcpy(b->data + (b->len * b->elem_size_bytes), &encoded, sizeof(encoded));
  b->len += 1;
}

void scoop_array_builder_push_composite(
    void *builder,
    const ScoopCompositeTransportDescriptor *descriptor,
    const void *value) {
  ScoopArrayBuilder *b = (ScoopArrayBuilder *)builder;
  if (!scoop_array_builder_configure(b, SCOOP_ARRAY_ELEM_KIND_COMPOSITE, descriptor)) {
    return;
  }

  if (b->len >= b->cap && !scoop_array_builder_grow_impl(b)) {
    return;
  }

  uint8_t *dst = b->data + (b->len * b->elem_size_bytes);
  if (value == 0) {
    (void)memset(dst, 0, (size_t)b->elem_size_bytes);
  } else {
    scoop_composite_copy(descriptor, dst, value);
  }
  b->len += 1;
}

static uint64_t scoop_array_allocation_size(
    uint64_t len,
    uint32_t elem_kind,
    const ScoopCompositeTransportDescriptor *desc,
    uint64_t *out_elem_size,
    uint64_t *out_data_offset) {
  uint64_t elem_size = scoop_array_element_size(elem_kind, desc);
  uint64_t elem_align = scoop_array_element_align(elem_kind, desc);
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

void *scoop_array_alloc(
    uint64_t len,
    uint64_t elem_kind_raw,
    const ScoopCompositeTransportDescriptor *descriptor) {
  uint32_t elem_kind = (uint32_t)elem_kind_raw;
  const ScoopCompositeTransportDescriptor *elem_desc =
      (elem_kind == SCOOP_ARRAY_ELEM_KIND_COMPOSITE) ? descriptor : 0;
  if (elem_kind != SCOOP_ARRAY_ELEM_KIND_WORD && elem_kind != SCOOP_ARRAY_ELEM_KIND_REF &&
      elem_kind != SCOOP_ARRAY_ELEM_KIND_COMPOSITE) {
    return 0;
  }

  uint64_t elem_size = 0;
  uint64_t data_offset = 0;
  uint64_t bytes = scoop_array_allocation_size(
      len, elem_kind, elem_desc, &elem_size, &data_offset);
  if (bytes == 0) {
    return 0;
  }

  ScoopArray *arr = (ScoopArray *)scoop_alloc_typed(&SCOOP_ARRAY_TYPE_DESC, bytes);
  if (arr == 0) {
    return 0;
  }

  arr->len = len;
  arr->elem_size_bytes = elem_size;
  arr->data_offset_bytes = data_offset;
  arr->elem_desc = elem_desc;
  arr->elem_kind = elem_kind;
  arr->_reserved_u32 = 0;
  if (bytes > data_offset) {
    (void)memset(((uint8_t *)arr) + data_offset, 0, (size_t)(bytes - data_offset));
  }
  return (void *)arr;
}

static void scoop_array_builder_reset_after_transfer(ScoopArrayBuilder *b) {
  if (b == 0) {
    return;
  }
  if (b->data != 0) {
    free(b->data);
  }
  b->data = 0;
  b->len = 0;
  b->cap = 0;
  b->elem_size_bytes = 0;
  b->elem_align_bytes = 0;
  b->elem_desc = 0;
  b->elem_kind = SCOOP_ARRAY_ELEM_KIND_UNKNOWN;
}

static void *scoop_array_builder_build_common(
    ScoopArrayBuilder *b,
    const ScoopCompositeTransportDescriptor *empty_composite_desc) {
  if (b == 0) {
    return 0;
  }

  uint32_t elem_kind = b->elem_kind;
  const ScoopCompositeTransportDescriptor *elem_desc = b->elem_desc;
  if (elem_kind == SCOOP_ARRAY_ELEM_KIND_UNKNOWN) {
    if (empty_composite_desc != 0) {
      elem_kind = SCOOP_ARRAY_ELEM_KIND_COMPOSITE;
      elem_desc = empty_composite_desc;
    } else {
      elem_kind = SCOOP_ARRAY_ELEM_KIND_WORD;
    }
  }

  uint64_t elem_size = 0;
  uint64_t data_offset = 0;
  uint64_t bytes = scoop_array_allocation_size(
      b->len,
      elem_kind,
      elem_desc,
      &elem_size,
      &data_offset);
  if (bytes == 0) {
    return 0;
  }

  scoop_pin((void *)b);
  ScoopArray *arr = (ScoopArray *)scoop_alloc(bytes);
  if (arr == 0) {
    scoop_unpin((void *)b);
    return 0;
  }
  scoop_pin((void *)arr);

  arr->header.type_desc = &SCOOP_ARRAY_TYPE_DESC;
  arr->len = 0;
  arr->elem_size_bytes = elem_size;
  arr->data_offset_bytes = data_offset;
  arr->elem_desc = (elem_kind == SCOOP_ARRAY_ELEM_KIND_COMPOSITE) ? elem_desc : 0;
  arr->elem_kind = elem_kind;
  arr->_reserved_u32 = 0;

  uint8_t *dst = scoop_array_data(arr);
  if (b->len > 0 && b->data != 0 && dst != 0) {
    if (elem_kind == SCOOP_ARRAY_ELEM_KIND_COMPOSITE && elem_desc != 0) {
      for (uint64_t i = 0; i < b->len; i++) {
        ScoopArrayPreparedCompositeCopy prepared;
        if (!scoop_array_prepare_composite_copy(
                elem_desc, b->data + (i * elem_size), &prepared)) {
          scoop_array_release((void *)arr);
          scoop_unpin((void *)arr);
          scoop_unpin((void *)b);
          return 0;
        }
        arr->len += 1;
        scoop_array_commit_composite_copy_to_gc_storage(dst + (i * elem_size), &prepared);
        scoop_array_destroy_prepared_composite_copy(&prepared);
      }
      scoop_array_builder_drop_elements(b);
    } else if (elem_kind == SCOOP_ARRAY_ELEM_KIND_REF) {
      for (uint64_t i = 0; i < b->len; i++) {
        void *value = 0;
        uint8_t *slot = dst + (i * elem_size);
        (void)memcpy(&value, b->data + (i * elem_size), sizeof(void *));
        (void)scoop_gc_write_barrier((void *)slot, value);
        arr->len += 1;
      }
    } else {
      (void)memcpy(dst, b->data, (size_t)b->len * (size_t)elem_size);
      arr->len = b->len;
    }
  }

  scoop_array_builder_reset_after_transfer(b);
  scoop_unpin((void *)arr);
  scoop_unpin((void *)b);
  return (void *)arr;
}

void *scoop_array_builder_build_array(void *builder) {
  return scoop_array_builder_build_common((ScoopArrayBuilder *)builder, 0);
}

void *scoop_array_builder_build_mutable_array(void *builder) {
  return scoop_array_builder_build_common((ScoopArrayBuilder *)builder, 0);
}

void *scoop_array_builder_build_array_composite(
    void *builder,
    const ScoopCompositeTransportDescriptor *descriptor) {
  return scoop_array_builder_build_common((ScoopArrayBuilder *)builder, descriptor);
}

void *scoop_array_builder_build_mutable_array_composite(
    void *builder,
    const ScoopCompositeTransportDescriptor *descriptor) {
  return scoop_array_builder_build_common((ScoopArrayBuilder *)builder, descriptor);
}
