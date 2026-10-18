#include "idna.h"
#include <unistring/version.h>

scoop_idna_result scoop_idna_error(int code, int64_t label) {
  return (scoop_idna_result){NULL, 0, code, label};
}

scoop_idna_result scoop_idna_bytes(uint8_t *bytes, size_t size) {
  return (scoop_idna_result){bytes, (int64_t)size, 0, -1};
}

void scoop_idna_free(uint8_t *data) { idn2_free(data); }

const char *scoop_idna_message(int32_t code) {
  switch (code) {
  case SCOOP_IDNA_EMPTY:
    return "empty domain name";
  case SCOOP_IDNA_LABEL:
    return "empty label or separator in a single label";
  case SCOOP_IDNA_ALLOC:
    return "IDNA allocation failed";
  case SCOOP_IDNA_DISALLOWED:
    return "domain contains a disallowed DNS hostname character";
  case SCOOP_IDNA_LENGTH:
    return "DNS wire name exceeds 255 bytes";
  case SCOOP_IDNA_SCALAR:
    return "Punycode does not encode Unicode scalar values";
  default:
    return idn2_strerror(code);
  }
}

const char *scoop_idna_version(void) {
  return "libidn2 " IDN2_VERSION
         " + Scoop ContextO/Bidi/NFC/Punycode; libunistring 1.4.2";
}
