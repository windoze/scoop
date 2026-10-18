#include "compression.h"

typedef union {
  size_t size;
  max_align_t alignment;
} comp_allocation;

void *comp_allocate(void *opaque, size_t size) {
  comp_codec *codec = opaque;
  if (size > SIZE_MAX - sizeof(comp_allocation)) {
    comp_fail(codec, COMP_OVERFLOW, 0, "compression allocation size overflow");
    return NULL;
  }
  size_t total = size + sizeof(comp_allocation);
  if (total > (uint64_t)INT64_MAX - codec->memory) {
    comp_fail(codec, COMP_OVERFLOW, 0, "compression allocation total overflow");
    return NULL;
  }
  comp_allocation *allocation = malloc(total);
  if (!allocation) {
    comp_fail(codec, COMP_MEMORY, ENOMEM, "cannot allocate compression memory");
    return NULL;
  }
  allocation->size = total;
  codec->memory += total;
  return allocation + 1;
}

void comp_release(void *opaque, void *address) {
  if (!address)
    return;
  comp_codec *codec = opaque;
  comp_allocation *allocation = (comp_allocation *)address - 1;
  codec->memory -= allocation->size;
  free(allocation);
}

void *comp_zallocate(void *opaque, unsigned int count, unsigned int size) {
  if (size && count > SIZE_MAX / size) {
    comp_fail(opaque, COMP_OVERFLOW, 0, "compression allocation size overflow");
    return NULL;
  }
  size_t total = (size_t)count * size;
  void *data = comp_allocate(opaque, total);
  if (data)
    memset(data, 0, total);
  return data;
}

void comp_fail(comp_codec *codec, int kind, int code, const char *detail) {
  if (codec->kind)
    return;
  codec->kind = kind;
  codec->code = code;
  snprintf(codec->detail, sizeof(codec->detail), "%s", detail);
}

comp_info scoop_compression_info(uint64_t handle) {
  comp_codec *codec = (comp_codec *)(uintptr_t)handle;
  return (comp_info){(int64_t)codec->memory, codec->kind, codec->code};
}

const char *scoop_compression_error(uint64_t handle) {
  return ((comp_codec *)(uintptr_t)handle)->detail;
}

const char *scoop_compression_version(int32_t format) {
  if (format <= 2)
    return ZLIBNG_VERSION;
  if (format == 3)
    return "1.2.0";
  return ZSTD_versionString();
}
