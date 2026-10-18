#include "idna.h"
#include <string.h>

static scoop_idna_result check_domain(uint8_t *domain, int label_only) {
  size_t size = strlen((char *)domain);
  if (!size)
    return scoop_idna_error(SCOOP_IDNA_EMPTY, -1);
  if (!label_only && size == 1 && domain[0] == '.')
    return scoop_idna_bytes(domain, size);
  if (size + (domain[size - 1] == '.' ? 1 : 2) > IDN2_DOMAIN_MAX_LENGTH)
    return scoop_idna_error(SCOOP_IDNA_LENGTH, -1);
  size_t start = 0;
  int64_t label = 0;
  for (size_t i = 0; i < size; ++i) {
    unsigned char byte = domain[i];
    if (byte == '.') {
      if (label_only || i == start)
        return scoop_idna_error(SCOOP_IDNA_LABEL, label);
      start = i + 1;
      ++label;
    } else if (!((byte >= 'a' && byte <= 'z') || (byte >= '0' && byte <= '9') ||
                 byte == '-')) {
      return scoop_idna_error(SCOOP_IDNA_DISALLOWED, label);
    }
  }
  return scoop_idna_bytes(domain, size);
}

scoop_idna_result scoop_idna_domain(const uint8_t *data, int64_t size,
                                    int32_t mode) {
  if (!size)
    return scoop_idna_error(SCOOP_IDNA_EMPTY, -1);
  for (int64_t i = 0; i < size; ++i)
    if (data[i] <= 32 || data[i] == 127)
      return scoop_idna_error(SCOOP_IDNA_DISALLOWED, -1);
  uint8_t *input = malloc((size_t)size + 1);
  if (!input)
    return scoop_idna_error(SCOOP_IDNA_ALLOC, -1);
  memcpy(input, data, (size_t)size);
  input[size] = 0;
  uint8_t *ascii = NULL;
  int code = idn2_lookup_u8(input, &ascii,
                            IDN2_NONTRANSITIONAL | IDN2_ALABEL_ROUNDTRIP);
  free(input);
  if (code)
    return scoop_idna_error(code, -1);
  scoop_idna_result result = check_domain(ascii, mode >= 2);
  if (result.code) {
    idn2_free(ascii);
    return result;
  }
  if (!(mode & 1))
    return result;
  char *unicode = NULL;
  code = idn2_to_unicode_8z8z((char *)ascii, &unicode, 0);
  idn2_free(ascii);
  if (code)
    return scoop_idna_error(code, -1);
  return scoop_idna_bytes((uint8_t *)unicode, strlen(unicode));
}

scoop_idna_name scoop_idna_parse(const uint8_t *data, int64_t size) {
  scoop_idna_result ascii = scoop_idna_domain(data, size, 0);
  if (ascii.code)
    return (scoop_idna_name){NULL, NULL, 0, 0, ascii.code, ascii.label};
  char *unicode = NULL;
  int code = idn2_to_unicode_8z8z((char *)ascii.data, &unicode, 0);
  if (code) {
    idn2_free(ascii.data);
    return (scoop_idna_name){NULL, NULL, 0, 0, code, -1};
  }
  return (scoop_idna_name){ascii.data, (uint8_t *)unicode,
                           ascii.size, (int64_t)strlen(unicode),
                           0,          -1};
}
