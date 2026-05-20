#include <scoop_runtime.h>

#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void scoop_lang_string_array_trap(const char *symbol) {
  (void)fprintf(stderr, "scoop.lang.string trap: invalid argument to %s\n", symbol);
  exit(3);
}

static void scoop_lang_string_require_mutable_array_shape(const void *arr,
                                                          uint32_t elem_kind,
                                                          uint64_t elem_size,
                                                          const char *symbol) {
  const uint64_t len = scoop_mutable_array_len(arr);
  if (arr == 0 || scoop_mutable_array_elem_kind(arr) != elem_kind ||
      scoop_mutable_array_elem_size(arr) != elem_size ||
      (len > 0 && scoop_mutable_array_to_array_data(arr) == 0)) {
    scoop_lang_string_array_trap(symbol);
  }
}

static uint32_t scoop_lang_string_normalize_unicode_scalar(int32_t codepoint) {
  uint32_t cp = (uint32_t)codepoint;
  if (cp > 0x10FFFFu || (cp >= 0xD800u && cp <= 0xDFFFu)) {
    return 0xFFFDu;
  }
  return cp;
}

static uint64_t scoop_lang_string_utf8_len_for_scalar(uint32_t cp) {
  if (cp <= 0x7Fu) {
    return 1;
  }
  if (cp <= 0x7FFu) {
    return 2;
  }
  if (cp <= 0xFFFFu) {
    return 3;
  }
  return 4;
}

static uint8_t *scoop_lang_string_emit_utf8_scalar(uint32_t cp, uint8_t *out) {
  if (out == 0) {
    return 0;
  }
  if (cp <= 0x7Fu) {
    out[0] = (uint8_t)cp;
    return out + 1;
  }
  if (cp <= 0x7FFu) {
    out[0] = (uint8_t)(0xC0u | (cp >> 6));
    out[1] = (uint8_t)(0x80u | (cp & 0x3Fu));
    return out + 2;
  }
  if (cp <= 0xFFFFu) {
    out[0] = (uint8_t)(0xE0u | (cp >> 12));
    out[1] = (uint8_t)(0x80u | ((cp >> 6) & 0x3Fu));
    out[2] = (uint8_t)(0x80u | (cp & 0x3Fu));
    return out + 3;
  }
  out[0] = (uint8_t)(0xF0u | (cp >> 18));
  out[1] = (uint8_t)(0x80u | ((cp >> 12) & 0x3Fu));
  out[2] = (uint8_t)(0x80u | ((cp >> 6) & 0x3Fu));
  out[3] = (uint8_t)(0x80u | (cp & 0x3Fu));
  return out + 4;
}

const ScoopString *scoop_string_from_byte_array(ScoopMutableArray *bytes) {
  scoop_lang_string_require_mutable_array_shape(
      bytes,
      SCOOP_ARRAY_ELEM_KIND_WORD,
      1u,
      "scoop_string_from_byte_array");

  const uint64_t len = scoop_mutable_array_len(bytes);
  if (len > (uint64_t)SIZE_MAX) {
    return 0;
  }

  scoop_pin((void *)bytes);
  if (len == 0) {
    const ScoopString *empty = scoop_string_from_owned_bytes(0, 0);
    scoop_unpin((void *)bytes);
    return empty;
  }

  const uint8_t *data = (const uint8_t *)scoop_mutable_array_to_array_data(bytes);
  uint8_t *out = (uint8_t *)malloc((size_t)len);
  if (out == 0) {
    scoop_unpin((void *)bytes);
    return 0;
  }
  (void)memcpy(out, data, (size_t)len);

  const ScoopString *result = scoop_string_from_owned_bytes(out, len);
  scoop_unpin((void *)bytes);
  return result;
}

const ScoopString *scoop_string_from_char_array(ScoopMutableArray *chars) {
  scoop_lang_string_require_mutable_array_shape(
      chars,
      SCOOP_ARRAY_ELEM_KIND_WORD,
      4u,
      "scoop_string_from_char_array");

  scoop_pin((void *)chars);
  const uint64_t len = scoop_mutable_array_len(chars);
  const uint8_t *data = (const uint8_t *)scoop_mutable_array_to_array_data(chars);
  uint64_t total = 0;
  for (uint64_t i = 0; i < len; i++) {
    uint32_t raw = 0;
    (void)memcpy(&raw, data + (i * 4u), sizeof(raw));
    uint64_t encoded_len = scoop_lang_string_utf8_len_for_scalar(
        scoop_lang_string_normalize_unicode_scalar((int32_t)raw));
    if (UINT64_MAX - total < encoded_len) {
      scoop_unpin((void *)chars);
      return 0;
    }
    total += encoded_len;
  }
  if (total > (uint64_t)SIZE_MAX) {
    scoop_unpin((void *)chars);
    return 0;
  }
  if (total == 0) {
    const ScoopString *empty = scoop_string_from_owned_bytes(0, 0);
    scoop_unpin((void *)chars);
    return empty;
  }

  uint8_t *out = (uint8_t *)malloc((size_t)total);
  if (out == 0) {
    scoop_unpin((void *)chars);
    return 0;
  }
  uint8_t *cursor = out;
  for (uint64_t i = 0; i < len; i++) {
    uint32_t raw = 0;
    (void)memcpy(&raw, data + (i * 4u), sizeof(raw));
    uint32_t cp = scoop_lang_string_normalize_unicode_scalar((int32_t)raw);
    cursor = scoop_lang_string_emit_utf8_scalar(cp, cursor);
  }

  const ScoopString *result = scoop_string_from_owned_bytes(out, total);
  scoop_unpin((void *)chars);
  return result;
}

const ScoopString *scoop_string_from_string_array(ScoopMutableArray *parts) {
  scoop_lang_string_require_mutable_array_shape(
      parts,
      SCOOP_ARRAY_ELEM_KIND_REF,
      (uint64_t)sizeof(void *),
      "scoop_string_from_string_array");

  scoop_pin((void *)parts);
  const uint64_t len = scoop_mutable_array_len(parts);
  const uint8_t *data = (const uint8_t *)scoop_mutable_array_to_array_data(parts);
  uint64_t total = 0;
  for (uint64_t i = 0; i < len; i++) {
    const ScoopString *part = 0;
    (void)memcpy(&part, data + (i * sizeof(void *)), sizeof(part));
    const uint64_t part_len = scoop_string_byte_length(part);
    if (part == 0 || part_len == 0) {
      continue;
    }
    if (scoop_string_bytes(part) == 0 || UINT64_MAX - total < part_len) {
      scoop_lang_string_array_trap("scoop_string_from_string_array");
    }
    total += part_len;
  }
  if (total > (uint64_t)SIZE_MAX) {
    scoop_unpin((void *)parts);
    return 0;
  }
  if (total == 0) {
    const ScoopString *empty = scoop_string_from_owned_bytes(0, 0);
    scoop_unpin((void *)parts);
    return empty;
  }

  uint8_t *out = (uint8_t *)malloc((size_t)total);
  if (out == 0) {
    scoop_unpin((void *)parts);
    return 0;
  }
  uint64_t offset = 0;
  for (uint64_t i = 0; i < len; i++) {
    const ScoopString *part = 0;
    (void)memcpy(&part, data + (i * sizeof(void *)), sizeof(part));
    const uint64_t part_len = scoop_string_byte_length(part);
    if (part == 0 || part_len == 0) {
      continue;
    }
    (void)memcpy(out + offset, scoop_string_bytes(part), (size_t)part_len);
    offset += part_len;
  }

  const ScoopString *result = scoop_string_from_owned_bytes(out, total);
  scoop_unpin((void *)parts);
  return result;
}
