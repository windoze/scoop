// Scoop runtime internal Array / MutableArray layout definitions.
//
// This header is private to runtime/c. It keeps the C layout shared between the
// array implementation and runtime helpers that need to inspect array metadata.

#ifndef SCOOP_ARRAY_INTERNAL_H
#define SCOOP_ARRAY_INTERNAL_H

#include <stdint.h>

#include "scoop_gc.h"

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

typedef struct ScoopMutableArray {
  ScoopGcObjectHeader header;
  uint64_t len;
  uint64_t cap;
  uint64_t elem_size_bytes;
  uint64_t elem_align_bytes;
  const ScoopCompositeTransportDescriptor *elem_desc;
  uint8_t *data;
  uint32_t elem_kind;
  uint32_t _reserved_u32;
} ScoopMutableArray;

#endif
