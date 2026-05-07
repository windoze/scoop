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

static const uint8_t *scoop_array_const_data(const ScoopArray *arr) {
  if (arr == 0 || arr->data_offset_bytes == 0) {
    return 0;
  }
  return ((const uint8_t *)arr) + arr->data_offset_bytes;
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
    .release_fn = 0,
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

static uint32_t scoop_array_builder_grow(ScoopArrayBuilder *b) {
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

  if (b->len >= b->cap && !scoop_array_builder_grow(b)) {
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

  if (b->len >= b->cap && !scoop_array_builder_grow(b)) {
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

  if (b->len >= b->cap && !scoop_array_builder_grow(b)) {
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
  arr->len = b->len;
  arr->elem_size_bytes = elem_size;
  arr->data_offset_bytes = data_offset;
  arr->elem_desc = (elem_kind == SCOOP_ARRAY_ELEM_KIND_COMPOSITE) ? elem_desc : 0;
  arr->elem_kind = elem_kind;
  arr->_reserved_u32 = 0;

  uint8_t *dst = scoop_array_data(arr);
  if (b->len > 0 && b->data != 0 && dst != 0) {
    if (elem_kind == SCOOP_ARRAY_ELEM_KIND_COMPOSITE && elem_desc != 0) {
      for (uint64_t i = 0; i < b->len; i++) {
        scoop_composite_copy(
            elem_desc,
            dst + (i * elem_size),
            b->data + (i * elem_size));
      }
      scoop_array_builder_drop_elements(b);
    } else {
      (void)memcpy(dst, b->data, (size_t)b->len * (size_t)elem_size);
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

uint64_t scoop_array_len(void *array_obj) {
  if (array_obj == 0) {
    return 0;
  }

  ScoopArray *arr = (ScoopArray *)array_obj;
  return arr->len;
}

uint64_t scoop_array_get_u64(void *array_obj, int64_t index) {
  if (array_obj == 0 || index < 0) {
    return 0;
  }

  ScoopArray *arr = (ScoopArray *)array_obj;
  if (arr->elem_kind != SCOOP_ARRAY_ELEM_KIND_WORD || arr->elem_size_bytes < sizeof(uint64_t)) {
    return 0;
  }
  uint64_t idx = (uint64_t)index;
  if (idx >= arr->len) {
    return 0;
  }

  uint64_t value = 0;
  const uint8_t *src = scoop_array_const_data(arr) + (idx * arr->elem_size_bytes);
  (void)memcpy(&value, src, sizeof(value));
  return value;
}

void *scoop_array_get_ref(void *array_obj, int64_t index) {
  if (array_obj == 0 || index < 0) {
    return 0;
  }

  ScoopArray *arr = (ScoopArray *)array_obj;
  if (arr->elem_kind != SCOOP_ARRAY_ELEM_KIND_REF || arr->elem_size_bytes < sizeof(uintptr_t)) {
    return 0;
  }
  uint64_t idx = (uint64_t)index;
  if (idx >= arr->len) {
    return 0;
  }

  uintptr_t encoded = 0;
  const uint8_t *src = scoop_array_const_data(arr) + (idx * arr->elem_size_bytes);
  (void)memcpy(&encoded, src, sizeof(encoded));
  return (void *)encoded;
}

void scoop_array_get_composite(
    void *array_obj,
    int64_t index,
    const ScoopCompositeTransportDescriptor *descriptor,
    void *out_value) {
  if (array_obj == 0 || descriptor == 0 || out_value == 0 || index < 0) {
    return;
  }
  ScoopArray *arr = (ScoopArray *)array_obj;
  if (arr->elem_kind != SCOOP_ARRAY_ELEM_KIND_COMPOSITE || arr->elem_desc != descriptor ||
      arr->elem_size_bytes != descriptor->size_bytes) {
    return;
  }
  uint64_t idx = (uint64_t)index;
  if (idx >= arr->len) {
    return;
  }

  scoop_pin((void *)arr);
  const uint8_t *src = scoop_array_const_data(arr) + (idx * arr->elem_size_bytes);
  scoop_composite_copy(descriptor, out_value, src);
  scoop_unpin((void *)arr);
}

void scoop_array_set_u64(void *array_obj, int64_t index, uint64_t value) {
  if (array_obj == 0 || index < 0) {
    return;
  }

  ScoopArray *arr = (ScoopArray *)array_obj;
  if (arr->elem_kind != SCOOP_ARRAY_ELEM_KIND_WORD || arr->elem_size_bytes < sizeof(uint64_t)) {
    return;
  }
  uint64_t idx = (uint64_t)index;
  if (idx >= arr->len) {
    return;
  }

  uint8_t *dst = scoop_array_data(arr) + (idx * arr->elem_size_bytes);
  (void)memcpy(dst, &value, sizeof(value));
}

void scoop_array_set_ref(void *array_obj, int64_t index, void *value) {
  if (array_obj == 0 || index < 0) {
    return;
  }

  ScoopArray *arr = (ScoopArray *)array_obj;
  if (arr->elem_kind != SCOOP_ARRAY_ELEM_KIND_REF || arr->elem_size_bytes < sizeof(uintptr_t)) {
    return;
  }
  uint64_t idx = (uint64_t)index;
  if (idx >= arr->len) {
    return;
  }

  uint8_t *slot = scoop_array_data(arr) + (idx * arr->elem_size_bytes);
  (void)scoop_gc_write_barrier((void *)slot, value);
}

static void scoop_array_composite_write_barriers(
    uint8_t *dst,
    const ScoopCompositeTransportDescriptor *descriptor) {
  if (dst == 0 || descriptor == 0 || descriptor->gc_slot_offsets == 0 ||
      descriptor->gc_slot_count == 0) {
    return;
  }
  for (uint32_t i = 0; i < descriptor->gc_slot_count; i++) {
    uint64_t raw_off = descriptor->gc_slot_offsets[i];
    if (raw_off > descriptor->size_bytes || descriptor->size_bytes - raw_off < sizeof(void *)) {
      continue;
    }
    void **slot = (void **)(dst + raw_off);
    (void)scoop_gc_write_barrier((void *)slot, *slot);
  }
}

void scoop_array_set_composite(
    void *array_obj,
    int64_t index,
    const ScoopCompositeTransportDescriptor *descriptor,
    const void *value) {
  if (array_obj == 0 || descriptor == 0 || value == 0 || index < 0) {
    return;
  }
  ScoopArray *arr = (ScoopArray *)array_obj;
  if (arr->elem_kind != SCOOP_ARRAY_ELEM_KIND_COMPOSITE || arr->elem_desc != descriptor ||
      arr->elem_size_bytes != descriptor->size_bytes) {
    return;
  }
  uint64_t idx = (uint64_t)index;
  if (idx >= arr->len) {
    return;
  }

  scoop_pin((void *)arr);
  uint8_t *dst = scoop_array_data(arr) + (idx * arr->elem_size_bytes);
  scoop_composite_drop(descriptor, dst);
  scoop_composite_copy(descriptor, dst, value);
  scoop_array_composite_write_barriers(dst, descriptor);
  scoop_unpin((void *)arr);
}
